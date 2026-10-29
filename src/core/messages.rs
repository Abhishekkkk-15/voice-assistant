use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessage, ChatCompletionRequestDeveloperMessage, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage
};

#[derive(Debug, Clone, Default)]
pub struct MessagesRegistry {
    messages: Vec<ChatCompletionRequestMessage>,
}

impl MessagesRegistry {
    /// Initialize with a system prompt
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![ChatCompletionRequestSystemMessage::from(system_prompt.into()).into()],
        }
    }

    /// Add a standard user message
    pub fn user(&mut self, text: impl Into<String>) {
        self.messages
            .push(ChatCompletionRequestUserMessage::from(text.into()).into());
    }

    /// Add a standard assistant text response
    pub fn assistant(&mut self, text: impl Into<String>) {
        self.messages
            .push(ChatCompletionRequestAssistantMessage::from(text.into()).into());
    }

    /// Add a developer instruction message
    pub fn developer(&mut self, text: impl Into<String>) {
        self.messages
            .push(ChatCompletionRequestDeveloperMessage::from(text.into()).into());
    }

    /// 1. Record the assistant's decision to call tools.
    /// This MUST be appended before adding tool responses.
    /// Supports optional text (e.g., filler speech: "Looking up flight...")
    pub fn assistant_with_tool_calls(
        &mut self,
        text: Option<String>,
        tool_calls: Vec<ChatCompletionMessageToolCalls>,
    ) {
        let mut msg = ChatCompletionRequestAssistantMessage::default();
        msg.content = text.map(Into::into);
        msg.tool_calls = Some(tool_calls);
        self.messages.push(msg.into());
    }

    /// 2. Record the tool output for a given tool_call_id.
    pub fn tool_response(
        &mut self,
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
    ) {
        let tool_msg = ChatCompletionRequestToolMessage {
            tool_call_id: tool_call_id.into(),
            content: content.into().into(),
        };
        self.messages.push(tool_msg.into());
    }

    /// Direct reference to messages for building the OpenAI request
    pub fn get_messages(&self) -> &[ChatCompletionRequestMessage] {
        &self.messages
    }

    /// Mutable reference if you need to manipulate messages directly
    pub fn get_messages_mut(&mut self) -> &mut Vec<ChatCompletionRequestMessage> {
        &mut self.messages
    }
}