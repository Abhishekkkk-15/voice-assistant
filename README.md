# Voice Assistant Prompt Generator

Generate a reusable `prompt/CONTEXT.md` from public profile links like GitHub, portfolio, LinkedIn, and X.

The script first scrapes public profile pages, then uses your `LLM_KEY` to synthesize a cleaner and more complete context file when available.

By default it writes directly to `prompt/CONTEXT.md`.

## Usage

```bash
python generate_context.py --github https://github.com/abhishekkkk-15 --portfolio https://abhishekkkk.in
```

You can add more links:

```bash
python generate_context.py \
  --github https://github.com/abhishekkkk-15 \
  --portfolio https://abhishekkkk.in \
  --linkedin https://www.linkedin.com/in/abhishek-jangid-3532b1323
```

Optional inputs:

- `--x` for an X/Twitter profile URL
- `--output` to write somewhere other than `prompt/CONTEXT.md`
- `--interactive` to enter links in the terminal instead of passing flags

If you run the script without any URL flags, it will also open the interactive prompts automatically.

If `LLM_KEY` is not available, the script still falls back to a local markdown generator.
