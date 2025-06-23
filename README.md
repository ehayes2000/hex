# hex

hex enables LLM-driven workflows with recursive tool usage. The AI can call specified functions ("tools") as directed in a conversation, chaining calls as needed (e.g., reading, writing, editing files, listing directories).

## Structure
- **types/**: Tool trait, async variant, toolsets.
- **offline_tools/**: Built-in sync file tools (list, read, write, edit).
- **client/**:
  - `cli/`: Terminal chat client, OpenAI streaming + tool loop.
  - `web/`: Web client (prototype).
- **main.rs**: CLI entrypoint.

## Usage

**Requirements:**
- Rust toolchain
- `OPENAI_API_KEY` in your environment

**Run:**

```sh
export OPENAI_API_KEY=sk-...
cargo run
```

## Extending
- Implement `Tool` or `AsyncTool` for new tools. Add to the toolset as needed.

## License
MIT
