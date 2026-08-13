from voice_assistant import VoiceAssistant
from fastapi import FastAPI,Response
from fastapi.responses import FileResponse
from pathlib import Path
import uvicorn
app = FastAPI()

@app.get("/")
def health():
    return {"health":"true"}

@app.post("/chat")
async def chat_completion(prompt:str):
    vs = VoiceAssistant()
    user_query = prompt
    llm_output = vs.ask_llm(user_query)
    vs.text_to_speech(llm_output,output_filename="output.mp3")
    
    return FileResponse(
        path=Path("./output.mp3"),
        filename="output.mp3",
        media_type="audio/mpeg"
    )

if __name__ == "__main__":
    uvicorn.run("main:app", host="127.0.0.1", port=8000, reload=True)

