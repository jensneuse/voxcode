use voxcode::audio::{compute_rms, downmix_to_mono, resample_to_mono_16khz};

// --- compute_rms ---

#[test]
fn rms_empty_returns_zero() {
    assert_eq!(compute_rms(&[]), 0.0);
}

#[test]
fn rms_single_sample() {
    let rms = compute_rms(&[0.5]);
    assert!((rms - 0.5).abs() < 1e-6);
}

#[test]
fn rms_known_constant_signal() {
    let samples = vec![0.5_f32; 100];
    let rms = compute_rms(&samples);
    assert!((rms - 0.5).abs() < 1e-6);
}

#[test]
fn rms_negative_values_same_as_positive() {
    let pos = compute_rms(&[0.3, 0.4, 0.5]);
    let neg = compute_rms(&[-0.3, -0.4, -0.5]);
    assert!((pos - neg).abs() < 1e-6);
}

// --- downmix_to_mono ---

#[test]
fn downmix_zero_channels_returns_empty() {
    assert_eq!(downmix_to_mono(&[1.0, 2.0, 3.0], 0), Vec::<f32>::new());
}

#[test]
fn downmix_mono_returns_identity() {
    assert_eq!(downmix_to_mono(&[1.0, 2.0, 3.0], 1), vec![1.0, 2.0, 3.0]);
}

#[test]
fn downmix_stereo_takes_first_channel() {
    let mono = downmix_to_mono(&[1.0, 0.2, 0.5, 0.1], 2);
    assert_eq!(mono, vec![1.0, 0.5]);
}

#[test]
fn downmix_three_channels_takes_first() {
    let mono = downmix_to_mono(&[1.0, 0.2, 0.3, 0.5, 0.6, 0.7], 3);
    assert_eq!(mono, vec![1.0, 0.5]);
}

#[test]
fn downmix_empty_input_returns_empty() {
    assert_eq!(downmix_to_mono(&[], 2), Vec::<f32>::new());
}

// --- resample_to_mono_16khz ---

#[test]
fn resample_empty_returns_empty() {
    let output = resample_to_mono_16khz(&[], 48_000).unwrap();
    assert!(output.is_empty());
}

#[test]
fn resample_16khz_passthrough() {
    let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    let output = resample_to_mono_16khz(&input, 16_000).unwrap();
    assert_eq!(output, input);
}

#[test]
fn resample_48khz_to_16khz_correct_length() {
    let input: Vec<f32> = (0..480)
        .map(|i| ((i as f32 / 480.0) * std::f32::consts::TAU).sin())
        .collect();

    let output = resample_to_mono_16khz(&input, 48_000).unwrap();
    assert_eq!(output.len(), 160);
}

#[test]
fn resample_output_samples_are_finite() {
    let input: Vec<f32> = (0..960)
        .map(|i| ((i as f32 / 960.0) * std::f32::consts::TAU).sin())
        .collect();

    let output = resample_to_mono_16khz(&input, 48_000).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn resample_very_short_input() {
    let input = vec![0.1_f32; 5];
    let result = resample_to_mono_16khz(&input, 48_000);
    assert!(result.is_ok());
}
