use crate::commands::demo;

pub(crate) async fn blank_play_creation_loop() -> anyhow::Result<()> {
    let (_store, runtime) = super::fixtures::runtime();
    demo::run_blank_play_creation_loop(&runtime)
        .await
        .map(|_| ())
}
