# Python service acceptance fixture

This deliberately small external-source fixture has a different shape from the
real static-site Work used by the Host operations acceptance test. It uses only
Python's standard library, has no Plurora manifest, and starts without a
Dockerfile. The acceptance workflow imports it into a contained managed
Workspace, adopts the resulting content-addressed Work as an Installation, then
adds that deployment description through an approved Host ChangeSet before the
service can be verified and deployed.
