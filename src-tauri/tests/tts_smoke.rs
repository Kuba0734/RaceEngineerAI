//! Manual smoke test: `SHERPA_TEST_VOICE_DIR=<dir with de_DE-thorsten-medium.onnx> cargo test --test tts_smoke -- --ignored`
#[test]
#[ignore]
fn synthesizes_german() {
    let Ok(dir) = std::env::var("SHERPA_TEST_VOICE_DIR") else { return };
    let config = sherpa_onnx::OfflineTtsConfig {
        model: sherpa_onnx::OfflineTtsModelConfig {
            vits: sherpa_onnx::OfflineTtsVitsModelConfig {
                model: Some(format!("{dir}/de_DE-thorsten-medium.onnx")),
                tokens: Some(format!("{dir}/tokens.txt")),
                data_dir: Some(format!("{dir}/espeak-ng-data")),
                ..Default::default()
            },
            num_threads: 2,
            debug: true,
            provider: Some("cpu".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let tts = sherpa_onnx::OfflineTts::create(&config).expect("create");
    let t = std::time::Instant::now();
    let cfg = sherpa_onnx::GenerationConfig { speed: 1.1, ..Default::default() };
    let audio = tts.generate_with_config("Box, Box, diese Runde. Sprit reicht nicht.", &cfg, None::<fn(&[f32], f32) -> bool>).expect("gen");
    let secs = audio.samples().len() as f32 / audio.sample_rate() as f32;
    println!("rendered {secs:.2}s audio in {:?} at {} Hz", t.elapsed(), audio.sample_rate());
    assert!(secs > 1.0);
}
