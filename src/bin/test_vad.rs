use silero_vad_crs::SileroVad;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vad = SileroVad::new()?;
    let chunk_size = 512;

    println!("\n=== Silero VAD Test ===");

    // Pure sine wave (440Hz)
    println!("--- Testing pure 440Hz sine wave ---");
    for i in 1..=5 {
        let chunk: Vec<f32> = (0..chunk_size)
            .map(|n| (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 16000.0).sin() * 0.5)
            .collect();
        let prob = vad.forward_chunk(&chunk)?;
        println!("Sine Chunk #{i:02} | Prob: {prob:.4}");
    }

    Ok(())
}
