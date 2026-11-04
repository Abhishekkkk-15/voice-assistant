use std::{io::{self, Write}, sync::Arc};
use tokio::sync::mpsc;
use voice_assistant::{
    core::{
        agent::{Agent, EXPRESSIVE_VOICE_SYSTEM_PROMPT},
        agent_loop::agent_loop,
        messages::MessagesRegistry,
        tools::{tavily::TavilySearchTool, tool_registory::ToolRegistory, tools::WeatherTool},
    },
    model::events::AssistantEvent,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    println!("==================================================");
    println!("🤖 Agent Loop Test CLI");
    println!("Type your message (e.g. 'What is the weather in Tokyo?' or 'Search for latest news on Rust')");
    println!("Type 'exit' to quit.");
    println!("==================================================\n");

    let mut agent = Agent::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);
    let mut memory = MessagesRegistry::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);

    // Channel for events emitted by agent loop
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<AssistantEvent>(32);
    let (tts_tx, mut tts_rx) = mpsc::channel::<String>(32);

    // Background listener for TTS clauses
    tokio::spawn(async move {
        while let Some(clause) = tts_rx.recv().await {
            // Can be monitored if needed
            let _ = clause;
        }
    });

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

    let mut tool_reg = ToolRegistory::new();
    tool_reg.register(Arc::new(WeatherTool));
    tool_reg.register(Arc::new(TavilySearchTool));
    let tool_registory_arc = Arc::new(tool_reg);

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
        let reg_clone = Arc::clone(&tool_registory_arc);
        if let Err(e) = agent_loop(&mut agent, trimmed.to_string(), outbound_tx.clone(), &mut memory, &tts_tx, reg_clone).await {
            eprintln!("Error in agent loop: {e}");
        }

        println!("\n--------------------------------------------------");
    }

    println!("Goodbye!");
    Ok(())
}
