//! `@exact/language-model`'s bake (LLP 1133.016 D8, D15 items 3 and 4): a
//! `shape-name` option names one of the app's shapes and goes as its JSON
//! Schema; `prompt`'s answer is the constraint's shape; the declaration's
//! new fields read. The package is the repository's own, installed beside
//! the app as `exact add` would.

use std::path::{Path, PathBuf};

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "exact-language-model-{name}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/language-model");
        let to = path.join("node_modules/@exact/language-model");
        for sub in ["", "web"] {
            std::fs::create_dir_all(to.join(sub)).unwrap();
            std::fs::copy(
                from.join(sub).join("exact.json"),
                to.join(sub).join("exact.json"),
            )
            .unwrap();
        }
        std::fs::copy(from.join("package.json"), to.join("package.json")).unwrap();
        std::fs::write(path.join("app.contract"), contract).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self) -> Result<exact_plan::Plan, String> {
        contract::compile_path_all(&self.0.join("app.contract"), false)
            .map(|(plan, _)| plan)
            .map_err(|e| format!("{e:?}"))
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const USE: &str = r#"use createLanguageModel, prompt, promptStreaming, languageModelResponse, LanguageModelTool, LanguageModelInfo, LanguageModelResult, LanguageModelResponseInfo, LanguageModelResponse from "@exact/language-model"

shape Title
  title: string
  tags: string[]
  tone: option<enum<"plain", "warm">>

shape NoteQuery
  query: string
"#;

fn app(body: &str) -> String {
    format!(
        "{USE}\ncomponent App\n  state session = \"\"\n  mutation made as shape LanguageModelInfo\n{body}  view\n    text \"\"\n"
    )
}

/// D8: the literal names a shape, and the plan carries its schema, each
/// field required unless an option, an enum a string's `enum`.
#[test]
fn a_shape_named_by_a_literal_goes_as_its_json_schema() {
    let a = App::new(
        "schema",
        &app(concat!(
            "  mutation titled as shape Title\n",
            "  action make\n",
            "    send made = createLanguageModel(tools=[LanguageModelTool(name=\"findNotes\", description=\"Finds notes\", inputSchema=\"NoteQuery\")])\n",
            "  action title\n",
            "    send titled = prompt(session=session, input=\"A title\", responseConstraint=\"Title\")\n",
        )),
    );
    let plan = a.compile().unwrap_or_else(|e| panic!("{e}"));
    let bytes = String::from_utf8_lossy(&plan.encode()).into_owned();
    let title = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "tone": {"type": "string", "enum": ["plain", "warm"]},
        },
        "required": ["title", "tags"],
    });
    let query = serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]});
    for schema in [title, query] {
        assert!(
            bytes.contains(&schema.to_string()),
            "{schema} is not in the plan"
        );
    }
}

/// D15 item 4: `prompt` with a constraint answers the named shape; without
/// one, `LanguageModelResult`; another shape is refused.
#[test]
fn a_constrained_prompt_answers_the_shape_it_names() {
    let plain = App::new(
        "plain",
        &app("  mutation said as shape LanguageModelResult\n  action say\n    send said = prompt(session=session, input=\"Hi\")\n"),
    );
    plain.compile().unwrap_or_else(|e| panic!("{e}"));
    let wrong = App::new(
        "wrong",
        &app("  mutation said as shape LanguageModelResult\n  action say\n    send said = prompt(session=session, input=\"Hi\", responseConstraint=\"Title\")\n"),
    );
    let e = wrong
        .compile()
        .expect_err("the constraint's shape is the answer");
    assert!(
        e.contains("contract-module-answers") && e.contains("Title"),
        "{e}"
    );
}

/// D8: a name that is no shape, and a computed one, are refused by name.
#[test]
fn a_shape_name_is_a_literal_naming_one_of_the_app_s_shapes() {
    let unknown = App::new(
        "unknown",
        &app("  mutation titled as shape Title\n  action title\n    send titled = prompt(session=session, input=\"A title\", responseConstraint=\"Titel\")\n"),
    );
    let e = unknown.compile().expect_err("no shape `Titel`");
    assert!(
        e.contains("contract-module-shape-name") && e.contains("Titel"),
        "{e}"
    );
    let computed = App::new(
        "computed",
        &app("  state which = \"Title\"\n  mutation titled as shape Title\n  action title\n    send titled = prompt(session=session, input=\"A title\", responseConstraint=which)\n"),
    );
    let e = computed.compile().expect_err("a computed shape name");
    assert!(e.contains("contract-module-shape-name"), "{e}");
}

/// D7, D15 item 3: a streamed response is read by a polled source whose
/// declaration polls only while the response generates.
#[test]
fn a_response_reader_compiles_with_its_poll() {
    let a = App::new(
        "poll",
        &app(concat!(
            "  mutation started as shape LanguageModelResponseInfo\n",
            "  derive id = match started { case some(s) => s.id, case none => \"\" }\n",
            "  resource reply = languageModelResponse(response=id) as shape LanguageModelResponse else empty()\n",
            "  action stream\n",
            "    send started = promptStreaming(session=session, input=\"Hi\")\n",
        )),
    );
    let plan = a.compile().unwrap_or_else(|e| panic!("{e}"));
    let bytes = String::from_utf8_lossy(&plan.encode()).into_owned();
    assert!(
        bytes.contains(r#""while":{"state":["generating","calling-tool"]}"#),
        "the substitute carries the poll's `while`"
    );
}
