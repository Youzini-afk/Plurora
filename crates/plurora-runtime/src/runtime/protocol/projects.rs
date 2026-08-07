use super::*;

impl<S> Runtime<S>
where
    S: EventStore,
{
    // --- Project ---

    fn ensure_project_access(
        context: &ProtocolContext,
        action: &str,
        project_id: &ProjectId,
        method: &str,
    ) -> anyhow::Result<()> {
        if !context.allows_host_action(action)
            || !context.allows_host_resource("host", "project", project_id.as_str())
        {
            anyhow::bail!(
                "{method} permission denied: authenticated authority does not include action '{action}' for project '{}'",
                project_id
            );
        }
        Ok(())
    }

    fn project_id_param(params: &Value, method: &str) -> anyhow::Result<ProjectId> {
        let id = params
            .get("project_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("{method} requires project_id"))?;
        ProjectId::new(id)
    }

    fn project_summary(entry: &crate::ProjectEntry) -> anyhow::Result<Value> {
        let storage_summary = Self::project_storage_summary(&entry.descriptor.project.id);
        let mut summary = json!({
            "id": entry.descriptor.project.id.as_str(),
            "title": entry.descriptor.project.title,
            "description": entry.descriptor.project.description,
            "type": serde_json::to_value(&entry.descriptor.project.project_type)?,
            "state": serde_json::to_value(entry.state)?,
            "icon": entry.descriptor.project.icon,
            "entry_surface_id": entry.descriptor.project.entry_surface_id,
            "storage_summary": storage_summary,
        });
        if let Value::Object(map) = &mut summary {
            if let Some(session_id) = entry
                .descriptor
                .project
                .metadata
                .get("running_session_id")
                .and_then(Value::as_str)
            {
                map.insert("running_session_id".to_string(), json!(session_id));
            }
        }
        Ok(summary)
    }

    fn project_storage_summary(id: &ProjectId) -> Value {
        match plurora_core::paths::project_dir(id)
            .map_err(anyhow::Error::from)
            .and_then(|path| Self::directory_size_no_follow(&path))
        {
            Ok(total_bytes) => json!({
                "data_bytes": total_bytes,
                "cache_bytes": 0,
                "bundle_bytes": 0,
                "log_bytes": 0,
                "total_bytes": total_bytes,
                "measured_at": chrono::Utc::now().to_rfc3339(),
                "measurement_state": "measured",
            }),
            Err(_) => json!({
                "data_bytes": null,
                "cache_bytes": null,
                "bundle_bytes": null,
                "log_bytes": null,
                "total_bytes": null,
                "measured_at": null,
                "measurement_state": "unknown",
            }),
        }
    }

    fn directory_size_no_follow(path: &Path) -> anyhow::Result<u64> {
        let metadata = fs::symlink_metadata(path)?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            return Ok(0);
        }
        if file_type.is_file() {
            return Ok(metadata.len());
        }
        if !file_type.is_dir() {
            return Ok(0);
        }

        let mut total = 0_u64;
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            total = total.saturating_add(Self::directory_size_no_follow(&entry.path())?);
        }
        Ok(total)
    }

    fn count_dir_entries(path: &std::path::Path) -> usize {
        fs::read_dir(path)
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    fn project_paths_and_counts(id: &ProjectId) -> Value {
        let project_dir = plurora_core::paths::project_dir(id).ok();
        let sessions_count = project_dir
            .as_ref()
            .map(|dir| Self::count_dir_entries(&dir.join("sessions")))
            .unwrap_or(0);
        let secrets_path = plurora_core::paths::project_secret_store_path(id).ok();
        let secrets_exists = secrets_path.as_ref().is_some_and(|path| path.is_file());
        let secrets_count = if secrets_exists { 1 } else { 0 };
        json!({
            "project_dir": project_dir.map(|path| path.display().to_string()),
            "secrets_exists": secrets_exists,
            "sessions_count": sessions_count,
            "secrets_count": secrets_count,
        })
    }

    pub(crate) async fn dispatch_project_list(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        if !context.allows_host_action("observe") {
            anyhow::bail!(
                "kernel.v1.project.list permission denied: authenticated authority lacks observe"
            );
        }
        let filter_state = params
            .get("filter_state")
            .map(|value| serde_json::from_value::<ProjectState>(value.clone()))
            .transpose()?;
        let projects = self
            .config
            .project_registry
            .list()
            .into_iter()
            .filter(|entry| {
                context.allows_host_resource(
                    "host",
                    "project",
                    entry.descriptor.project.id.as_str(),
                )
            })
            .filter(|entry| filter_state.map_or(true, |state| entry.state == state))
            .map(|entry| Self::project_summary(&entry))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(json!({ "projects": projects }))
    }

    pub(crate) async fn dispatch_project_get(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let id = Self::project_id_param(params, "kernel.v1.project.get")?;
        Self::ensure_project_access(context, "observe", &id, "kernel.v1.project.get")?;
        let entry = self
            .config
            .project_registry
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found", id))?;
        let mut value = serde_json::to_value(&entry.descriptor)?;
        if let Value::Object(map) = &mut value {
            map.insert("state".to_string(), serde_json::to_value(entry.state)?);
            map.insert(
                "storage_summary".to_string(),
                Self::project_storage_summary(&id),
            );
            if matches!(entry.state, ProjectState::Running | ProjectState::Starting) {
                if let Some(session_id) = self.find_session_for_project(&id).await {
                    map.insert("running_session_id".to_string(), json!(session_id));
                }
            }
        }
        Ok(value)
    }

    pub(crate) async fn dispatch_project_status(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let id = Self::project_id_param(params, "kernel.v1.project.status")?;
        Self::ensure_project_access(context, "observe", &id, "kernel.v1.project.status")?;
        let entry = self
            .config
            .project_registry
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found", id))?;
        let details = Self::project_paths_and_counts(&id);
        let mut value = json!({
            "project_id": id.as_str(),
            "state": serde_json::to_value(entry.state)?,
            "sessions_count": details.get("sessions_count").and_then(Value::as_u64).unwrap_or(0),
            "secrets_count": details.get("secrets_count").and_then(Value::as_u64).unwrap_or(0),
            "storage_summary": Self::project_storage_summary(&id),
        });
        if matches!(entry.state, ProjectState::Running | ProjectState::Starting) {
            if let Some(session_id) = self.find_session_for_project(&id).await {
                if let Value::Object(map) = &mut value {
                    map.insert("running_session_id".to_string(), json!(session_id));
                }
            }
        }
        Ok(value)
    }

    pub(crate) async fn dispatch_project_start(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let id = Self::project_id_param(params, "kernel.v1.project.start")?;
        Self::ensure_project_access(context, "project_operate", &id, "kernel.v1.project.start")?;
        let entry = self
            .config
            .project_registry
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found", id))?;
        let previous_state = entry.state;

        if matches!(entry.state, ProjectState::Running | ProjectState::Starting) {
            if let Some(existing_session_id) = self.find_session_for_project(&id).await {
                return Ok(json!({
                    "project_id": id.as_str(),
                    "previous_state": serde_json::to_value(previous_state)?,
                    "new_state": serde_json::to_value(entry.state)?,
                    "session_id": existing_session_id,
                    "already_running": true,
                }));
            }
        }

        if matches!(entry.state, ProjectState::Archived) {
            anyhow::bail!("project '{}' is archived; restore before starting", id);
        }

        if !matches!(
            entry.state,
            ProjectState::Installed | ProjectState::Stopped | ProjectState::Failed
        ) {
            anyhow::bail!("project '{}' cannot start from state {:?}", id, entry.state);
        }

        self.config
            .project_registry
            .set_state(&id, ProjectState::Starting)?;

        let session = self
            .open_session(OpenSessionRequest {
                labels: vec![format!("project:{}", id.as_str())],
                metadata: json!({
                    "project_id": id.as_str(),
                    "project_title": entry.descriptor.project.title,
                    "project_type": serde_json::to_value(&entry.descriptor.project.project_type)?,
                }),
                ..OpenSessionRequest::default()
            })
            .await?;
        let session_id = session.id.clone();

        self.append_kernel_event(
            &session_id,
            plurora_core::PROJECT_STARTED,
            json!({
                "project_id": entry.descriptor.project.id.as_str(),
                "title": entry.descriptor.project.title,
                "type": serde_json::to_value(&entry.descriptor.project.project_type)?,
                "previous_state": serde_json::to_value(previous_state)?,
                "new_state": serde_json::to_value(ProjectState::Running)?,
                "session_id": session_id,
            }),
        )
        .await?;

        self.config
            .project_registry
            .set_state(&id, ProjectState::Running)?;
        Ok(json!({
            "project_id": id.as_str(),
            "previous_state": serde_json::to_value(previous_state)?,
            "new_state": serde_json::to_value(ProjectState::Running)?,
            "session_id": session_id,
            "already_running": false,
        }))
    }

    pub(crate) async fn dispatch_project_stop(
        &self,
        context: &ProtocolContext,
        params: &Value,
    ) -> anyhow::Result<Value> {
        let id = Self::project_id_param(params, "kernel.v1.project.stop")?;
        Self::ensure_project_access(context, "project_operate", &id, "kernel.v1.project.stop")?;
        let entry = self
            .config
            .project_registry
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found", id))?;
        if !matches!(entry.state, ProjectState::Running | ProjectState::Starting) {
            anyhow::bail!("project '{}' cannot stop from state {:?}", id, entry.state);
        }
        let previous_state = entry.state;
        let session_id = self.find_session_for_project(&id).await;
        self.config
            .project_registry
            .set_state(&id, ProjectState::Stopping)?;

        if let Some(session_id) = &session_id {
            self.append_kernel_event(
                session_id,
                plurora_core::PROJECT_STOPPED,
                json!({
                    "project_id": entry.descriptor.project.id.as_str(),
                    "title": entry.descriptor.project.title,
                    "type": serde_json::to_value(&entry.descriptor.project.project_type)?,
                    "previous_state": serde_json::to_value(previous_state)?,
                    "new_state": serde_json::to_value(ProjectState::Stopped)?,
                    "session_id": session_id,
                }),
            )
            .await?;
            self.close_session(session_id.clone()).await?;
        }

        self.config
            .project_registry
            .set_state(&id, ProjectState::Stopped)?;
        Ok(json!({
            "project_id": id.as_str(),
            "previous_state": serde_json::to_value(previous_state)?,
            "new_state": serde_json::to_value(ProjectState::Stopped)?,
            "session_id": session_id,
        }))
    }
}
