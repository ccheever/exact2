use super::{body, clean, has_lifetime, strip, Delimiter, TokenStream, TokenTree};

pub(super) fn expand(input: TokenStream) -> Result<String, String> {
    let tokens: Vec<_> = input.into_iter().collect();
    let (tokens, _) = strip(&tokens, false)?;
    if tokens.first().is_none_or(|t| t.to_string() != "struct") {
        return Err("Kind requires a nongeneric named struct".into());
    }
    let name = tokens[1].to_string();
    if !matches!(tokens.get(2), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace)
        || has_lifetime(tokens)
        || tokens.iter().any(|t| t.to_string() == "where")
    {
        return Err(format!("{name}: Kind requires a nongeneric named struct"));
    }
    let fields = body(tokens.get(2))?.fields;
    if fields.is_empty() || fields.len() > 8 {
        return Err(format!("{name}: Kind requires 1..=8 component fields"));
    }
    let mut insert = String::new();
    let mut check = String::new();
    let mut declarations = [String::new(), String::new()];
    let mut queries = [String::new(), String::new()];
    let mut views = String::new();
    let mut singles = [String::new(), String::new()];
    for (i, f) in fields.iter().enumerate() {
        if f.skip {
            return Err(format!("{name}: Kind fields cannot use data(skip)"));
        }
        let field = &f.name;
        if field == "id" {
            return Err(format!("{name}: id is reserved for the typed row handle"));
        }
        let tokens: Vec<_> = f.ty.clone().into_iter().collect();
        // Recognize Option with a qualified or unqualified path; retain all
        // inner tokens, including nested generic component types.
        let open = tokens.iter().position(|t| super::punct(t, '<'));
        let optional = open.is_some_and(|i| i > 0 && tokens[i - 1].to_string() == "Option");
        let ty = if optional {
            tokens[open.unwrap() + 1..tokens.len() - 1]
                .iter()
                .cloned()
                .collect::<TokenStream>()
                .to_string()
        } else {
            f.ty.to_string()
        };
        if optional {
            insert +=
                &format!("if let Some(value) = self.{field} {{ world.insert(entity, value); }}");
        } else {
            insert += &format!("world.insert(entity, self.{field});");
            check += &format!("if !world.has::<{ty}>(entity) {{ return Err(::exact_game::KindError::missing::<Self, {ty}>(entity)); }}");
        }
        views += &format!("{field}: values.{i},");
        for mode in 0..2 {
            let mutable = mode == 1 && !f.shared;
            let guard = if mutable { "RefMut" } else { "Ref" };
            let reference = if mutable { "&'static mut" } else { "&'static" };
            let get = if mutable { "get_mut" } else { "get" };
            let (view, query, single) = if optional {
                (
                    format!("Option<::exact_game::{guard}<'w, {ty}>>"),
                    format!("Option<{reference} {ty}>"),
                    format!("world.{get}::<{ty}>(entity)"),
                )
            } else {
                (format!("::exact_game::{guard}<'w, {ty}>"),
                 format!("{reference} {ty}"), format!("world.{get}::<{ty}>(entity).ok_or_else(|| ::exact_game::KindError::missing::<Self, {ty}>(entity))?"))
            };
            declarations[mode] += &format!(
                "#[doc = {:?}] pub {field}: {view},",
                format!("The {} component.", clean(field))
            );
            queries[mode] += &format!("{query},");
            singles[mode] += &format!("{field}: {single},");
        }
    }
    let read = format!("{}Ref", clean(&name));
    let write = format!("{}Mut", clean(&name));
    Ok(format!(
        "#[doc = \"Shared component leases.\"] pub struct {read}<'w> {{ #[doc = \"Entity represented by this row.\"] pub id: ::exact_game::Id<{name}>, {} }}
         #[doc = \"Component leases for editing; read fields remain shared.\"] pub struct {write}<'w> {{ #[doc = \"Entity represented by this row.\"] pub id: ::exact_game::Id<{name}>, {} }}
         impl ::exact_game::Kind for {name} {{
             type Ref<'w> = {read}<'w>; type Mut<'w> = {write}<'w>;
             type Read = ({}); type Write = ({});
             fn insert(self, world: &mut ::exact_game::World, entity: ::exact_game::Entity) {{ {insert} }}
             fn check(world: &::exact_game::World, entity: ::exact_game::Entity) -> Result<(), ::exact_game::KindError> {{ {check} Ok(()) }}
             fn view<'w>(id: ::exact_game::Id<Self>, values: <Self::Read as ::exact_game::Query>::Owned<'w>) -> Self::Ref<'w> {{ {read} {{ id, {views} }} }}
             fn view_mut<'w>(id: ::exact_game::Id<Self>, values: <Self::Write as ::exact_game::Query>::Owned<'w>) -> Self::Mut<'w> {{ {write} {{ id, {views} }} }}
             fn row(world: &::exact_game::World, id: ::exact_game::Id<Self>) -> Result<Self::Ref<'_>, ::exact_game::KindError> {{ let entity = id.entity(); Ok({read} {{ id, {} }}) }}
             fn row_mut(world: &::exact_game::World, id: ::exact_game::Id<Self>) -> Result<Self::Mut<'_>, ::exact_game::KindError> {{ let entity = id.entity(); Ok({write} {{ id, {} }}) }}
         }}",
        declarations[0], declarations[1], queries[0], queries[1], singles[0], singles[1]
    ))
}
