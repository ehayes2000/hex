# Recursive Streaming in the CLI Client (Pseudocode)

Below is a detailed step-by-step pseudocode outlining the recursive streaming sequence implemented in the CLI client. This pseudocode abstracts the main control flow, including streaming responses and recursive handling of tool function calls.

---

```pseudocode
function chat():
    clear_screen()
    while True:
        user_input = read_user_input()
        messages.append(create_user_message(user_input))
        chat_response()

function chat_response():
    stream = send_chat_message(messages)  // returns streaming reply from API
    parsed_stream = parse_stream(stream) // yields StreamItem::Content or StreamItem::ToolCall
    items = stdout_stream(parsed_stream) // print content as streamed, collect all items
    new_messages = process_stream(items)

    if new_messages contains Tool message:
        messages.extend(new_messages)
        chat_response() // recursive call: send new tool outputs to assistant
    else:
        print_newline()
        messages.extend(new_messages)
        return

function send_chat_message(messages):
    request = build_chat_request(messages)
    return openai.create_stream(request)

function parse_stream(stream):
    for each event in stream:
        if event is content token:
            yield StreamItem::Content(text)
        if event is function/tool call fragment:
            accumulate to PartialCall
        if function/tool call finished:
            yield StreamItem::ToolCall(complete_call)

function stdout_stream(parsed_stream):
    items = []
    for item in parsed_stream:
        if item is StreamItem::Content:
            print(item.text)
        items.append(item)
    return items

function process_stream(items):
    messages = []
    for item in items:
        if item is ToolCall:
            tool_response = execute_tool(item)
            messages.append(create_tool_message(tool_response))
        else if item is Content:
            assistant_reply += item.text
    if assistant_reply not empty:
        messages.prepend(create_assistant_message(assistant_reply))
    return messages
```

---

### Notes
- The recursion inside `chat_response()` ensures chains of function/tool calls are fully handled before the user is prompted again.
- Streaming enables prompt printing of assistant output, while the recursive model enables correct context handling for advanced agentic interactions.
