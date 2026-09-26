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
  tags: list<string>
  reply: option<Author>

component App
  resource posts = feed() as shape list<Post>
  view
    text `${length(posts)}`
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
