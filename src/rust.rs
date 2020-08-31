use maplit::btreeset;
use proc_macro2::Span;
use std::{collections::BTreeSet, mem};
use syn::{
    parse_quote, spanned::Spanned, Ident, Item, ItemUse, Meta, PathSegment, UseGroup, UseName,
    UseRename, UseTree,
};

pub(crate) struct Equipment {
    pub(crate) extern_crate_name: Ident,
    pub(crate) mods: Option<BTreeSet<Ident>>,
    pub(crate) uses: Vec<ItemUse>,
    pub(crate) span: Span,
}

#[allow(clippy::type_complexity)]
pub(crate) fn parse_exactly_one_use(file: &syn::File) -> syn::Result<Option<Equipment>> {
    let mut uses = vec![];

    for item in &file.items {
        if let Item::Use(item_use) = item {
            if let Some((i, meta)) = item_use
                .attrs
                .iter()
                .enumerate()
                .flat_map(|(i, a)| a.parse_meta().map(|m| (i, m)))
                .find(|(_, meta)| {
                    matches!(
                        meta.path().segments.first(),
                        Some(PathSegment { ident, .. }) if ident == "cargo_equip"
                    )
                })
            {
                let span = item_use.span();

                if meta
                    .path()
                    .segments
                    .iter()
                    .map(|PathSegment { ident, .. }| ident)
                    .collect::<Vec<_>>()
                    != ["cargo_equip", "equip"]
                {
                    return Err(syn::Error::new(span, "expected `cargo_equip::equip`"));
                }

                if let Meta::List(_) | Meta::NameValue(_) = meta {
                    return Err(syn::Error::new(
                        span,
                        "`cargo_equip::equip` take no argument",
                    ));
                }

                let mut item_use = item_use.clone();
                item_use.attrs.remove(i);
                uses.push((item_use, span));
            }
        }
    }

    if uses.len() > 1 {
        return Err(syn::Error::new(file.span(), "multiple `cargo_equip` usage"));
    }

    let (item_use, span) = if let Some(target) = uses.pop() {
        target
    } else {
        return Ok(None);
    };

    if item_use.leading_colon.is_none() {
        return Err(syn::Error::new(
            item_use.tree.span(),
            "leading semicolon (`::`) is requied",
        ));
    }

    let new_item_use = |tree| ItemUse {
        leading_colon: None,
        tree,
        ..item_use.clone()
    };

    let use_path = match &item_use.tree {
        UseTree::Path(use_path) => use_path,
        _ => {
            return Err(syn::Error::new(
                item_use.tree.span(),
                "expected `$ident::$tree`",
            ));
        }
    };

    let (mods, uses) = match &*use_path.tree {
        UseTree::Path(use_path) => {
            let mods = Some(btreeset!(use_path.ident.clone()));
            let uses = vec![new_item_use(parse_quote!(self::#use_path))];
            (mods, uses)
        }
        UseTree::Name(UseName { ident }) => {
            let mods = Some(btreeset!(ident.clone()));
            let uses = vec![];
            (mods, uses)
        }
        UseTree::Rename(UseRename { ident, rename, .. }) => {
            let mods = Some(btreeset!(ident.clone()));
            let uses = vec![new_item_use(parse_quote!(self::#ident as #rename))];
            (mods, uses)
        }
        UseTree::Glob(_) => {
            let mods = None;
            let uses = vec![];
            (mods, uses)
        }
        UseTree::Group(UseGroup { items, .. }) => {
            let mut flatten = items.iter().collect::<Vec<_>>();
            while flatten.iter().any(|x| matches!(x, UseTree::Group(_))) {
                for item in mem::take(&mut flatten) {
                    if let UseTree::Group(UseGroup { items, .. }) = item {
                        flatten.extend(items);
                    } else {
                        flatten.push(item);
                    }
                }
            }
            let (mut mods, mut uses) = (Some(btreeset![]), vec![]);
            for item in flatten {
                match item {
                    UseTree::Path(use_path) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(use_path.ident.clone());
                        }
                        uses.push(new_item_use(parse_quote!(self::#use_path)));
                    }
                    UseTree::Name(UseName { ident }) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(ident.clone());
                        }
                    }
                    UseTree::Rename(UseRename { ident, rename, .. }) => {
                        if let Some(mods) = &mut mods {
                            mods.insert(ident.clone());
                        }
                        uses.push(new_item_use(parse_quote!(self::#ident as #rename)));
                    }
                    UseTree::Glob(_) => {
                        mods = None;
                    }
                    UseTree::Group(_) => {
                        unreachable!("should be flatten");
                    }
                }
            }
            (mods, uses)
        }
    };

    Ok(Some(Equipment {
        extern_crate_name: use_path.ident.clone(),
        mods,
        uses,
        span,
    }))
}
