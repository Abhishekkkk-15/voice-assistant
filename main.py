from voice_assistant import VoiceAssistant
from fastapi import FastAPI, Response, UploadFile, File, HTTPException
from pathlib import Path
import uvicorn
from fastapi.middleware.cors import CORSMiddleware

app = FastAPI()
origins = [
    "http://localhost:3000",    
    "http://localhost:5173",     
    "http://localhost:8001",     
    "https://yourfrontend.com",  
]
app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,            
    allow_credentials=True,           
    allow_methods=["*"],              
    allow_headers=["*"],              
)

vs = VoiceAssistant()

@app.get("/")
def health():
    return {"health":"true"}

@app.post("/chat")
async def chat_completion(prompt: str):
    if not prompt.strip():
        raise HTTPException(status_code=400, detail="Prompt cannot be empty")

    llm_output = vs.ask_llm(prompt)
    
    # Get raw MP3 bytes in memory without saving to disk
    audio_bytes = vs.text_to_speech(llm_output, output_filename=None)

    return Response(
        content=audio_bytes,
        media_type="audio/mpeg",
        headers={"Content-Disposition": "inline; filename=output.mp3"}
    )

@app.post("/auto-chat")
async def audio_chat_completion(file:UploadFile = File(...)):
    try:
        audio_stream = await file.read()
        user_query = vs.speech_to_text(audio_stream, file_name=file.filename or "input.mp3")
        if not user_query.strip():
            user_query = "Hello, I sent an audio recording."
        llm_output = vs.ask_llm(user_query)
        audio_bytes = vs.text_to_speech(llm_output, output_filename=None)
        return Response(
            content=audio_bytes,
            media_type="audio/mpeg",
            headers={
                "Content-Disposition": "inline; filename=output.mp3",
                "X-User-Transcript": user_query  # Useful header to get transcript in frontend
            }
        )
    except Exception as e:   
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    uvicorn.run("main:app", host="127.0.0.1", port=8000, reload=True)

