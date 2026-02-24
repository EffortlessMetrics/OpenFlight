# flight-ac7-input

`flight-ac7-input` manages AC7 controller bindings by generating and patching
`Input.ini` with a Flight Hub managed block.

## Responsibilities

- Provide a typed AC7 input profile model.
- Render UE-style axis/action mapping lines for `Input.ini`.
- Apply idempotent managed-block updates without overwriting user content.
- Offer path helpers for AC7 config/save locations on Windows.

## Scope

- This crate only edits user config files.
- It does not inject input into game process memory.
