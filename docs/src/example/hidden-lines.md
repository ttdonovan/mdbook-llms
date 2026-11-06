---
description: Code lines hidden in the HTML are kept, without their marker, in the Markdown output
---

# Hidden lines

mdBook can hide boilerplate lines in code blocks. In the HTML they are folded away behind a toggle; in the llms output they are kept as ordinary code with the marker removed, so an agent gets complete, valid code.

The rules match mdBook's:

- Rust: a line starting with `# ` (or a lone `#`) is hidden, and `##` stands for a literal `#`.
- Other languages: lines starting with the prefix set in `[output.html.code.hidelines]`, or with a fence attribute like `hidelines=~`.

## Source

````markdown
```rust
# use std::collections::HashMap;
# fn main() {
let mut counts = HashMap::new();
counts.insert("pages", 7);
let heading = r#"
## Pages
"#;
# }
```

```python hidelines=~
~import json
print(json.dumps({"pages": 7}))
```
````

## Result

The same blocks, rendered by this book. The HTML page hides the marked lines; this page's `.md` file and `llms-full.txt` show them in full.

```rust
# use std::collections::HashMap;
# fn main() {
let mut counts = HashMap::new();
counts.insert("pages", 7);
let heading = r#"
## Pages
"#;
# }
```

```python hidelines=~
~import json
print(json.dumps({"pages": 7}))
```
