from voice_assistant import VoiceAssistant

vs = VoiceAssistant()
user_query = input("Enter yours query : ")
llm_output = vs.ask_llm(user_query)
vs.text_to_speech(llm_output,output_filename="output.mp3")
