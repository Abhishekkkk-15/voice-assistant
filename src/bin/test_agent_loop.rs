use std::io::{self, Write};
use tokio::sync::mpsc;
use voice_assistant::{
    core::{
        agent::{Agent, EXPRESSIVE_VOICE_SYSTEM_PROMPT},
        agent_loop::agent_loop,
        messages::MessagesRegistry,
    },
    model::events::AssistantEvent,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    println!("==================================================");
    println!("🤖 Agent Loop Test CLI");
    println!("Type your message (e.g. 'What is the weather in Tokyo?')");
    println!("Type 'exit' to quit.");
    println!("==================================================\n");

    let agent = Agent::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);
    let memory = MessagesRegistry::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);

    // Channel for events emitted by agent loop
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<AssistantEvent>(32);

    // Background listener for events
    tokio::spawn(async move {
        while let Some(event) = outbound_rx.recv().await {
            match event {
                AssistantEvent::Error(err) => eprintln!("\n❌ Assistant Error: {err}"),
                AssistantEvent::AiResponse { text, emotion } => {
                    println!("\n💬 [{emotion}]: {text}");
                }
                _ => {}
            }
        }
    });

    let stdin = io::stdin();
    loop {
        print!("You > ");
        io::stdout().flush()?;

        let mut input = String::new();
        if stdin.read_line(&mut input)? == 0 {
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("exit") {
            break;
        }

        println!("\n⏳ Agent running loop...\n");

        let agent = Agent::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);
        let memory_clone = memory.clone();

        if let Err(e) = agent_loop(agent, trimmed.to_string(), outbound_tx.clone(), memory_clone).await {
            eprintln!("Error in agent loop: {e}");
        }

        println!("\n--------------------------------------------------");
    }

    println!("Goodbye!");
    Ok(())
}
