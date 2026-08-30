//! LLP 1019 D1–D3/§5: declared static fonts lower to plan identity, and
//! every portable refusal happens before a host can substitute a font.

use exact_kernel::{FontStyle, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

struct AppDir(PathBuf);

impl AppDir {
    fn new(source: &str) -> AppDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-contract-fonts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts");
        for name in ["DejaVuSans.ttf", "DejaVuSans-Bold.ttf"] {
            std::fs::copy(
                fixtures.join("assets").join(name),
                dir.join("assets").join(name),
            )
            .unwrap();
        }
        std::fs::write(dir.join("app.contract"), source).unwrap();
        AppDir(dir)
    }

    fn compile(&self) -> Result<exact_plan::Plan, contract::CompileError> {
        contract::compile_path(&self.0.join("app.contract"))
    }
}

impl Drop for AppDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const DECLARATION: &str = "font \"Fixture Sans\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\n";

#[test]
fn declared_faces_and_stack_identity_reach_the_kernel_row() {
    let app = AppDir::new(&format!(
        "{DECLARATION}component App\n  view\n    text \"Change station\" font-family=\"Fixture Sans\" font-weight=600 font-style=\"normal\" testId=\"label\"\n"
    ));
    let plan = app.compile().unwrap();
    assert_eq!(plan.faces.len(), 2);
    assert_eq!(plan.str(plan.faces[0].source), "assets/DejaVuSans.ttf");
    assert_eq!(plan.str(plan.families[0].name), "Fixture Sans");
    assert_eq!(plan.stacks.len(), 9);

    let runner = Runner::boot(plan, NoData, Kernel::with_monospace()).unwrap();
    let kernel = runner.kernel();
    let node = kernel
        .node_by_key(kernel.find_by_test_id("label")[0])
        .unwrap();
    assert_eq!(node.style.font_family, 8);
    assert_eq!(node.style.font_weight, 600);
    assert_eq!(node.style.font_style, FontStyle::Normal);
}

#[test]
fn shorthand_declares_one_400_normal_face() {
    let app = AppDir::new(
        "font \"Fixture Sans\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"hello\" font-family=\"Fixture Sans\"\n",
    );
    let plan = app.compile().unwrap();
    assert_eq!(plan.faces.len(), 1);
    assert_eq!(plan.faces[0].weight, 400);
    assert!(!plan.faces[0].italic);
}

fn refusal(source: &str, id: &str) {
    let app = AppDir::new(source);
    let error = app.compile().unwrap_err();
    assert_eq!(error.id, id, "{error}");
}

#[test]
fn family_identity_and_sources_fail_closed() {
    refusal(
        "component App\n  view\n    text \"x\" font-family=\"Missing\"\n",
        "lower-font-undeclared",
    );
    refusal(
        "component App\n  view\n    text \"x\" font-family=\"serif, sans-serif\"\n",
        "lower-font-family-list",
    );
    refusal(
        "component App\n  state family = \"serif\"\n  view\n    text \"x\" font-family=family\n",
        "lower-font-family-literal",
    );
    refusal(
        "font \"Missing\" = \"assets/nope.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-unreadable",
    );
    refusal(
        "font \"Web Only\" = \"assets/font.woff2\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-format",
    );
    refusal(
        "font \"Escape\" = \"../DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-path",
    );
    refusal(
        "font \"Root\" = \"DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-path",
    );
}

#[test]
fn a_literal_request_that_would_need_synthesis_is_refused() {
    refusal(
        "font \"Regular Only\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\" font-family=\"Regular Only\" font-weight=700\n",
        "lower-font-face",
    );
    refusal(
        &format!(
            "{DECLARATION}component App\n  view\n    text \"x\" font-family=\"Fixture Sans\" font-style=\"italic\"\n"
        ),
        "lower-font-face",
    );
    refusal(
        "font \"Italic Only\"\n  400 italic = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\" font-family=\"Italic Only\" font-style=\"normal\"\n",
        "lower-font-face",
    );
}

#[test]
fn duplicate_direct_attributes_cannot_bypass_the_face_diagnostic() {
    refusal(
        "font \"HasBold\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\nfont \"OnlyRegular\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"hi\" font-family=\"HasBold\" font-family=\"OnlyRegular\" font-weight=700\n",
        "syntax-duplicate-attr",
    );
}

#[test]
fn duplicate_style_attributes_cannot_bypass_the_face_diagnostic() {
    refusal(
        "font \"HasBold\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\nfont \"OnlyRegular\" = \"assets/DejaVuSans.ttf\"\nstyle Dup\n  font-family=\"HasBold\"\n  font-family=\"OnlyRegular\"\n  font-weight=700\ncomponent A\n  view\n    text \"hi\" class=Dup font-size=13\n",
        "syntax-duplicate-attr",
    );
}
