import base64
import io
import mimetypes
import os
from pathlib import Path
from typing import BinaryIO
from dotenv import load_dotenv
from mistralai.client import Mistral
from mistralai.client.models.file import File

load_dotenv()


class VoiceAssistant:
    client: Mistral

    def __init__(self) -> None:
        api_key: str | None = os.getenv("LLM_KEY")
        if not api_key:
            raise ValueError("LLM API key is required")
        self.client = Mistral(api_key=api_key)

    def speech_to_text(
        self, audio_input: str | Path | bytes | BinaryIO, file_name: str = "audio.mp3"
    ) -> str:
        file_bytes: bytes
        name: str = file_name

        if isinstance(audio_input, (str, Path)):
            file_path = Path(audio_input)
            if not file_path.exists():
                raise FileNotFoundError(f"Audio file not found: {file_path}")
            file_bytes = file_path.read_bytes()
            name = file_path.name
        elif isinstance(audio_input, bytes):
            file_bytes = audio_input
        elif hasattr(audio_input, "read"):
            content = audio_input.read()
            if isinstance(content, str):
                file_bytes = content.encode("utf-8")
            else:
                file_bytes = content
        else:
            raise TypeError("Invalid audio input type. Expected str, Path, bytes, or file stream.")

        content_type, _ = mimetypes.guess_type(name)
        if not content_type:
            content_type = "audio/mpeg"

        response = self.client.audio.transcriptions.complete(
            model="voxtral-mini-latest",
            file=File(
                file_name=name,
                content=file_bytes,
                content_type=content_type,
            ),
        )

        if not response or response.text is None:
            raise RuntimeError("Failed to transcribe audio or received empty result.")

        return response.text

    def text_to_speech(
        self,
        text_prompt: str,
        output_filename: str | Path | None = None,
        voice_id: str = "530e2e20-58e2-45d8-b0a5-4594f4915944",
    ) -> bytes:

        response = self.client.audio.speech.complete(
            model="voxtral-mini-tts-2603",
            input=text_prompt,
            voice_id=voice_id,
            response_format="mp3",
        )

        raw_audio_bytes = base64.b64decode(response.audio_data)

        if output_filename:
            output_path = Path(output_filename)
            output_path.write_bytes(raw_audio_bytes)
            print(f"Saved audio to {output_path}")

        return raw_audio_bytes

    def ask_llm(self, user_query: str) -> str:
        if not user_query.strip():
            raise ValueError("User query is required")

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

        choice = response.choices[0] if response.choices else None
        if choice is None or choice.message is None or choice.message.content is None:
            raise RuntimeError("Received empty response from Mistral LLM")

        return str(choice.message.content)