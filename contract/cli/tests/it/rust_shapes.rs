//! `contract rust` (LLP 1054.000 R4): a struct per shape, its fields in
//! declaration order, converting to and from the runner's positional record.

const CONTRACT: &str = r#"
shape Author
  handle: string
  displayName: string

shape Post
  text: string
  type: string
  likeCount: number
  liked: bool
  author: Author
  tags: string[]
  reply: option<Author>

component App
  resource posts = feed() as shape Post[]
  view
    text `${posts.length}`
"#;

#[test]
fn a_struct_per_shape_in_declaration_order() {
    let rust = contract::rust(&contract::compile(CONTRACT).unwrap()).unwrap();
    let post = &rust[rust.find("pub struct Post {").unwrap()..];
    let post = &post[..post.find('}').unwrap()];
    let fields: Vec<&str> = post.lines().skip(1).map(str::trim).collect();
    assert_eq!(
        fields,
        [
            "pub text: String,",
            "pub r#type: String,",
            "pub like_count: f64,",
            "pub liked: bool,",
            "pub author: Author,",
            "pub tags: Vec<String>,",
            "pub reply: Option<Author>,",
        ]
    );
    assert!(rust.contains(
        "pub struct Author {\n    pub handle: String,\n    pub display_name: String,\n}"
    ));
    // The record is written in that order, and read back field by field.
    let to = &rust[rust.find("impl ContractValue for Post").unwrap()..];
    let order: Vec<usize> = [
        "self.text",
        "self.r#type",
        "self.like_count",
        "self.liked",
        "self.author",
        "self.tags",
        "self.reply",
    ]
    .iter()
    .map(|f| to.find(f).unwrap())
    .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    assert!(to.contains("like_count: ContractValue::from_value(&fields[2])?,"));
}

#[test]
fn rust_names_escape_keywords_and_refuse_colliding_fields() {
    let source = "shape type\n  value: number\ncomponent App\n  state row = type(value=1)\n  view\n    text \"x\"\n";
    let rust = contract::rust(&contract::compile(source).unwrap()).unwrap();
    assert!(rust.contains("pub struct r#type {"));
    assert!(rust.contains("impl ContractValue for r#type {"));
    let source = "shape Row\n  a-b: number\n  a_b: number\ncomponent App\n  state row = Row(a-b=1, a_b=2)\n  view\n    text \"x\"\n";
    let error = contract::rust(&contract::compile(source).unwrap()).unwrap_err();
    assert!(
        error.contains("a-b") && error.contains("a_b") && error.contains("Row"),
        "{error}"
    );
}

#[test]
fn both_rust_printers_refuse_type_names_that_collide_in_one_scope() {
    use exact_plan::{builder::PlanBuilder, TypesId};
    let source = "shape A-B\n  x: number\nshape A_B\n  y: string\ncomponent App\n  state one = A-B(x=1)\n  state two = A_B(y=\"two\")\n  view\n    text \"x\"\n";
    let plan = contract::compile(source).unwrap();
    let check = |result: Result<String, String>| {
        let error = result.unwrap_err();
        assert!(error.contains("A-B") && error.contains("A_B"), "{error}");
    };
    check(contract::rust(&plan));
    for separate in [false, true] {
        let mut b = PlanBuilder::from_plan(plan.clone());
        for (name, extension) in [
            ("A-B", "one"),
            ("A_B", if separate { "two" } else { "one" }),
        ] {
            let ty = TypesId(
                plan.types
                    .iter()
                    .position(|t| plan.str(t.name) == name)
                    .unwrap() as u32,
            );
            let source = b.source(name, &[], ty);
            b.module(source, extension, name, false, false, "never", "", "{}");
        }
        let binding = contract::module_rust(&b.finish().unwrap());
        if separate {
            assert_eq!(binding.unwrap().matches("pub struct A_B {").count(), 2);
        } else {
            check(binding);
        }
    }
}
