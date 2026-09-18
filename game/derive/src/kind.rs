use super::{body, clean, has_lifetime, strip, Delimiter, TokenStream, TokenTree};

pub(super) fn expand(input: TokenStream) -> Result<String, String> {
    let all: Vec<_> = input.into_iter().collect();
    let (tokens, _) = strip(&all, false)?;
    if tokens.first().is_none_or(|t| t.to_string() != "struct") {
        return Err("Kind requires a named struct, not an enum or union".into());
    }
    let name = tokens[1].to_string();
    match tokens.get(2) {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
            return Err(format!(
                "{name}: Kind does not support tuple structs; use named fields"
            ))
        }
        Some(t) if super::punct(t, ';') => {
            return Err(format!(
                "{name}: Kind does not support unit structs; use named fields"
            ))
        }
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => (),
        _ => {
            return Err(format!(
                "{name}: Kind does not support generic structs or where clauses"
            ))
        }
    }
    if has_lifetime(tokens) {
        return Err(format!("{name}: Kind fields cannot contain lifetimes"));
    }
    let mut vis = String::new();
    for (i, t) in all.iter().enumerate() {
        if t.to_string() == "pub" {
            vis = "pub".into();
            if let Some(TokenTree::Group(g)) = all.get(i + 1) {
                if g.delimiter() == Delimiter::Parenthesis {
                    vis += &g.to_string();
                }
            }
            break;
        }
        if t.to_string() == "struct" {
            break;
        }
    }
    let fields = body(tokens.get(2))?.fields;
    if fields.is_empty() || fields.len() > 8 {
        return Err(format!(
            "{name}: Kind requires 1..=8 component fields (maximum 8)"
        ));
    }
    let mut insert = String::new();
    let mut check = String::new();
    let mut preflight = String::new();
    let mut saved_checks = String::new();
    let mut leases = String::new();
    let mut bindings = String::new();
    let mut joined_bindings = String::new();
    let mut prepare_spawn = String::new();
    let mut attach = String::new();
    let mut declarations = [String::new(), String::new()];
    let mut queries = [String::new(), String::new()];
    let mut views = String::new();
    let mut singles = [String::new(), String::new()];
    let mut types = std::collections::BTreeSet::new();
    let mut required = false;
    for (i, f) in fields.iter().enumerate() {
        if f.skip {
            return Err(format!("{name}: Kind fields cannot use data(skip)"));
        }
        let field = &f.name;
        if clean(field) == "id" {
            return Err(format!("{name}: id is reserved for the typed row handle"));
        }
        let tokens: Vec<_> = f.ty.clone().into_iter().collect();
        let open = tokens.iter().position(|t| super::punct(t, '<'));
        let optional = open.is_some_and(|i| i > 0 && tokens[i - 1].to_string() == "Option");
        let ty = if optional {
            let inner = &tokens[open.unwrap() + 1..tokens.len() - 1];
            if inner.is_empty() || inner.iter().any(|t| t.to_string() == "Option") {
                return Err(format!(
                    "{name}.{field}: expected Option<Component>, not empty or nested Option"
                ));
            }
            inner.iter().cloned().collect::<TokenStream>().to_string()
        } else {
            f.ty.to_string()
        };
        if !types.insert(ty.clone()) {
            return Err(format!("{name}: duplicate component type {ty}; Kind fields must have distinct component types"));
        }
        preflight += &format!("(::core::any::TypeId::of::<{ty}>(), stringify!({field}), ::core::any::type_name::<{ty}>()),");
        leases += &format!(
            "world.kind_lease::<Self, {ty}>(entity, operation, mutable && {})?;",
            !f.shared
        );
        if optional {
            if f.child.is_some() {
                return Err(format!(
                    "{name}.{field}: child binding requires a required component"
                ));
            }
            insert +=
                &format!("if let Some(value) = self.{field} {{ world.insert(entity, value); }}");
        } else {
            required = true;
            insert += &format!("world.insert(entity, self.{field});");
            check += &format!("if !world.has::<{ty}>(entity) {{ return Err(::exact_game::KindError::missing::<Self, {ty}>(world, entity, operation)); }}");
        }
        if let Some((child, member)) = &f.child {
            saved_checks += &format!(
                "::exact_game::KindError::saved_field::<Self, {ty}>({:?})?;",
                clean(member)
            );
            prepare_spawn += &format!("self.{field}.{member} = world.kind_spawn_child::<Self, _>(name, {child}, self.{field}.{member})?;");
            attach += &format!("let child = world.get::<{ty}>(entity).expect(\"inserted component\").{member}; world.insert(child.entity(), ::exact_game::Parent(entity));");
            joined_bindings += &format!("world.kind_child::<Self, _>(entity, {child}, concat!(stringify!({field}), \".\", stringify!({member})), values.{i}.{member}, false, operation)?;");
            bindings += &format!(
                r#"
                let old = world.get::<{ty}>(entity).ok_or_else(|| ::exact_game::KindError::missing::<Self, {ty}>(world, entity, operation))?.{member};
                let resolved = world.kind_child::<Self, _>(entity, {child}, concat!(stringify!({field}), ".", stringify!({member})), old, initialize, operation)?;
                if resolved != old {{ world.get_mut::<{ty}>(entity).expect("checked component").{member} = resolved; }}
            "#
            );
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
                (format!("::exact_game::{guard}<'w, {ty}>"), format!("{reference} {ty}"), format!("world.{get}::<{ty}>(entity).ok_or_else(|| ::exact_game::KindError::missing::<Self, {ty}>(world, entity, operation))?"))
            };
            declarations[mode] += &format!("#[doc = \"Component lease.\"] pub {field}: {view},");
            queries[mode] += &format!("{query},");
            singles[mode] += &format!("{field}: {single},");
        }
    }
    if !required {
        return Err(format!(
            "{name}: Kind requires at least one required component"
        ));
    }
    let read = format!("{}Ref", clean(&name));
    let write = format!("{}Mut", clean(&name));
    let component_names = format!("{:?}", types.into_iter().collect::<Vec<_>>().join(", "));
    let has_bindings = fields.iter().any(|f| f.child.is_some());
    // Anonymous scope prevents collisions with user-authored HeroRef/HeroMut.
    Ok(format!(
        r#"
        const _: () = {{
         #[doc = "Shared component leases."] {vis} struct {read}<'w> {{ #[doc = "Typed entity."] pub id: ::exact_game::Id<{name}>, {} }}
         #[doc = "Editing component leases."] {vis} struct {write}<'w> {{ #[doc = "Typed entity."] pub id: ::exact_game::Id<{name}>, {} }}
         impl ::exact_game::Kind for {name} {{
             const COMPONENTS: &'static str = {component_names};
             const HAS_BINDINGS: bool = {has_bindings};
             type Ref<'w> = {read}<'w>; type Mut<'w> = {write}<'w>;
             type Read = ({}); type Write = ({});
             fn prepare_spawn(&mut self, world: &::exact_game::World, name: &str) -> Result<(), ::exact_game::KindError> {{ let _ = (world, name); {prepare_spawn} Ok(()) }}
             fn attach(world: &mut ::exact_game::World, entity: ::exact_game::Entity) {{ let _ = (&world, entity); {attach} }}
             fn preflight() -> Result<(), ::exact_game::KindError> {{ ::exact_game::KindError::unique::<Self>(&[{preflight}])?; {saved_checks} Ok(()) }}
             fn leases(world: &::exact_game::World, entity: Option<::exact_game::Entity>, operation: &str, mutable: bool) -> Result<(), ::exact_game::KindError> {{ {leases} Ok(()) }}
             fn joined_bindings(world: &::exact_game::World, entity: ::exact_game::Entity, values: <Self::Read as ::exact_game::Query>::Item<'_>, operation: &str) -> Result<(), ::exact_game::KindError> {{ let _ = (world, entity, &values, operation); {joined_bindings} Ok(()) }}
             fn insert(self, world: &mut ::exact_game::World, entity: ::exact_game::Entity) {{ {insert} }}
             fn check(world: &::exact_game::World, entity: ::exact_game::Entity, operation: &str) -> Result<(), ::exact_game::KindError> {{ {check} Ok(()) }}
             fn bindings(world: &::exact_game::World, entity: ::exact_game::Entity, initialize: bool, operation: &str) -> Result<(), ::exact_game::KindError> {{ let _ = (world, entity, initialize, operation); {bindings} Ok(()) }}
             fn view<'w>(id: ::exact_game::Id<Self>, values: <Self::Read as ::exact_game::Query>::Owned<'w>) -> Self::Ref<'w> {{ {read} {{ id, {views} }} }}
             fn view_mut<'w>(id: ::exact_game::Id<Self>, values: <Self::Write as ::exact_game::Query>::Owned<'w>) -> Self::Mut<'w> {{ {write} {{ id, {views} }} }}
             fn row<'w>(world: &'w ::exact_game::World, id: ::exact_game::Id<Self>, operation: &str) -> Result<Self::Ref<'w>, ::exact_game::KindError> {{ let entity = id.entity(); Ok({read} {{ id, {} }}) }}
             fn row_mut<'w>(world: &'w ::exact_game::World, id: ::exact_game::Id<Self>, operation: &str) -> Result<Self::Mut<'w>, ::exact_game::KindError> {{ let entity = id.entity(); Ok({write} {{ id, {} }}) }}
         }}
        }};
        "#,
        declarations[0], declarations[1], queries[0], queries[1], singles[0], singles[1]
    ))
}
