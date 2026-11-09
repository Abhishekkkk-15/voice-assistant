use silero_vad_crs::SileroVad;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vad = SileroVad::new()?;
    let chunk_size = 512;

    println!("\n Simple VAD Playground");

    for i in 1..=5 {
        let silence_chunk = vec![0.0f32; chunk_size];
        let prob = vad.forward_chunk(&silence_chunk)?;
        println!(
            "Chunk #{i:02} | Prob: {prob:.4} | s{}",
            if prob > 0.5 {
                "🗣️ SPEECH"
            } else {
                "🤫 silence"
            }
        );
    }

    Ok(())
}
