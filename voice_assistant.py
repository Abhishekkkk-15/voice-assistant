import os
from mistralai.client import Mistral, OptionalNullable
from mistralai.client import Models
import base64
from dotenv import load_dotenv
from pathlib import Path

load_dotenv()


class VoiceAssistant:
    client:  Mistral
    
    def __init__(self) -> None:
        api_key:str|None = os.getenv("LLM_KEY") 
        if not api_key:
            Exception("LLM API key is required")
        self.client = Mistral(api_key=api_key)
        
    def text_to_speech(self, text_prompt:str ,output_filename:str, voice_id =  "530e2e20-58e2-45d8-b0a5-4594f4915944") -> Path | None:
        response = self.client.audio.speech.complete(
            model="voxtral-mini-tts-2603",
            input=text_prompt,
            voice_id=voice_id,
            response_format="mp3",
            )
        output_path = Path(output_filename)
        with open(output_path, "wb") as f:
            f.write(base64.b64decode(response.audio_data))
        print("Saved to output.mp3")
        return None
        
    def ask_llm(self, user_query: str) -> str:
        if not user_query.strip():
            raise ValueError("User query is required")  # Added 'raise' keyword
    
        response = self.client.chat.complete(
            model="mistral-small-latest",
            messages=[
                {
                    "role": "system",
                    "content": (
                        "You are a helpful voice assistant. Keep answers brief, conversational, "
                        "and avoid using markdown symbols like stars or hashes since your text will be spoken aloud."
                    ),
                },
                {"role": "user", "content": user_query},
            ],
        )
    
        # Safely unpack the message to satisfy Pylance
        choice = response.choices[0] if response.choices else None
        if choice is None or choice.message is None or choice.message.content is None:
            raise RuntimeError("Received empty response from Mistral LLM")
    
        return str(choice.message.content)