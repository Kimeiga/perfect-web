//! The `pw` value type model.
//!
//! Deliberately owned by us rather than delegated to a backend. E0 measured that
//! Koka erases single-field value structs (`Money_usd(350)` **is** `350`) and
//! represents both `Nothing` and `Nil` as `null`, so nominal identity and the
//! Option/List distinction cannot survive a round trip through its JS output.
//! See ADR-0011.

use std::collections::BTreeMap;

/// Index into [`Program::adts`].
pub type AdtId = usize;
/// Index into [`Program::opaques`].
pub type OpaqueId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Bool,
    Int,
    Str,
    /// A user-declared algebraic data type.
    Adt(AdtId),
    Tuple(Vec<Type>),
    /// A nominal wrapper. Structurally identical to its representation type and
    /// *erased in optimized payloads*, but a distinct type everywhere in the
    /// checker and a distinct slot in the ABI schema.
    Opaque(OpaqueId),
}

impl Type {
    /// Types with unboundedly many values can never have a complete constructor
    /// signature, so a match over them always needs a wildcard arm.
    pub fn is_infinite(&self) -> bool {
        matches!(self, Type::Int | Type::Str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctor {
    pub name: String,
    pub fields: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adt {
    pub name: String,
    pub ctors: Vec<Ctor>,
}

/// A nominal opaque type: `opaque type StoreId = String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opaque {
    pub name: String,
    /// What it is represented as at runtime. NOT what it is *typed* as.
    pub representation: Box<Type>,
}

/// Everything the checker needs to know about declared types.
#[derive(Debug, Clone, Default)]
pub struct Program {
    pub adts: Vec<Adt>,
    pub opaques: Vec<Opaque>,
    by_adt_name: BTreeMap<String, AdtId>,
    by_opaque_name: BTreeMap<String, OpaqueId>,
}

impl Program {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn declare_adt(&mut self, name: &str, ctors: Vec<Ctor>) -> AdtId {
        let id = self.adts.len();
        self.adts.push(Adt {
            name: name.to_string(),
            ctors,
        });
        self.by_adt_name.insert(name.to_string(), id);
        id
    }

    pub fn declare_opaque(&mut self, name: &str, representation: Type) -> OpaqueId {
        let id = self.opaques.len();
        self.opaques.push(Opaque {
            name: name.to_string(),
            representation: Box::new(representation),
        });
        self.by_opaque_name.insert(name.to_string(), id);
        id
    }

    pub fn adt(&self, id: AdtId) -> &Adt {
        &self.adts[id]
    }

    pub fn opaque(&self, id: OpaqueId) -> &Opaque {
        &self.opaques[id]
    }

    pub fn adt_id(&self, name: &str) -> Option<AdtId> {
        self.by_adt_name.get(name).copied()
    }

    pub fn opaque_id(&self, name: &str) -> Option<OpaqueId> {
        self.by_opaque_name.get(name).copied()
    }

    /// The constructors of `ty`, if it has a finite, enumerable set.
    ///
    /// An opaque type does NOT expose its representation's constructors: that is
    /// the whole point of opacity. `Opaque(StoreId)` over `Str` is not matchable
    /// as a string.
    pub fn ctors_of(&self, ty: &Type) -> Option<Vec<Ctor>> {
        match ty {
            Type::Bool => Some(vec![
                Ctor {
                    name: "false".into(),
                    fields: vec![],
                },
                Ctor {
                    name: "true".into(),
                    fields: vec![],
                },
            ]),
            Type::Adt(id) => Some(self.adt(*id).ctors.clone()),
            Type::Tuple(elems) => Some(vec![Ctor {
                name: "(,)".into(),
                fields: elems.clone(),
            }]),
            Type::Int | Type::Str | Type::Opaque(_) => None,
        }
    }

    pub fn type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "Bool".into(),
            Type::Int => "Int".into(),
            Type::Str => "String".into(),
            Type::Adt(id) => self.adt(*id).name.clone(),
            Type::Opaque(id) => self.opaque(*id).name.clone(),
            Type::Tuple(elems) => format!(
                "({})",
                elems
                    .iter()
                    .map(|t| self.type_name(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    // --- the platform prelude ------------------------------------------------
    //
    // Charter §7.1 requires `Option<T>` and `Result<T, E>`. E0 measured that
    // Koka supplies neither in a usable form: `error<a>` fixes the error side to
    // `exception`, and `Nothing`/`Nil` are both `null`. So the platform owns
    // them. ADR-0011 §5.

    /// `type Option<T> = None | Some(T)` — monomorphised at `inner`.
    pub fn declare_option(&mut self, inner: Type) -> AdtId {
        let name = format!("Option<{}>", self.type_name(&inner));
        if let Some(id) = self.adt_id(&name) {
            return id;
        }
        self.declare_adt(
            &name,
            vec![
                Ctor {
                    name: "None".into(),
                    fields: vec![],
                },
                Ctor {
                    name: "Some".into(),
                    fields: vec![inner],
                },
            ],
        )
    }

    /// `type List<T> = Nil | Cons(T, List<T>)` — monomorphised at `inner`.
    ///
    /// Declared separately from `Option` on purpose. Both are `null`-headed in
    /// Koka's JS output; here they are distinct nominal types and the ABI
    /// decoder is told which one it is decoding.
    pub fn declare_list(&mut self, inner: Type) -> AdtId {
        let name = format!("List<{}>", self.type_name(&inner));
        if let Some(id) = self.adt_id(&name) {
            return id;
        }
        let id = self.declare_adt(
            &name,
            vec![Ctor {
                name: "Nil".into(),
                fields: vec![],
            }],
        );
        // `Cons(T, List<T>)` is recursive, so the tail type needs the id we just
        // allocated.
        self.adts[id].ctors.push(Ctor {
            name: "Cons".into(),
            fields: vec![inner, Type::Adt(id)],
        });
        id
    }

    /// `type Result<T, E> = Ok(T) | Error(E)` — the platform-owned version.
    pub fn declare_result(&mut self, ok: Type, err: Type) -> AdtId {
        let name = format!("Result<{}, {}>", self.type_name(&ok), self.type_name(&err));
        if let Some(id) = self.adt_id(&name) {
            return id;
        }
        self.declare_adt(
            &name,
            vec![
                Ctor {
                    name: "Ok".into(),
                    fields: vec![ok],
                },
                Ctor {
                    name: "Error".into(),
                    fields: vec![err],
                },
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_types_do_not_expose_their_representation() {
        let mut p = Program::new();
        let store_id = p.declare_opaque("StoreId", Type::Str);
        // A `StoreId` is represented as a string but is NOT matchable as one.
        assert!(p.ctors_of(&Type::Opaque(store_id)).is_none());
        assert_eq!(p.type_name(&Type::Opaque(store_id)), "StoreId");
        assert_ne!(Type::Opaque(store_id), Type::Str);
    }

    #[test]
    fn two_opaques_over_the_same_representation_are_different_types() {
        // The case Koka erases: `Money_usd(350)` and a plain `350` become the
        // same JS value. Here they never become the same type.
        let mut p = Program::new();
        let usd = p.declare_opaque("Money<USD>", Type::Int);
        let eur = p.declare_opaque("Money<EUR>", Type::Int);
        assert_ne!(Type::Opaque(usd), Type::Opaque(eur));
        assert_ne!(Type::Opaque(usd), Type::Int);
    }

    #[test]
    fn option_and_list_are_distinct_types() {
        // Both are `null`-headed in Koka's JS output and runtime-ambiguous there.
        let mut p = Program::new();
        let opt = p.declare_option(Type::Int);
        let list = p.declare_list(Type::Int);
        assert_ne!(opt, list);
        assert_eq!(p.type_name(&Type::Adt(opt)), "Option<Int>");
        assert_eq!(p.type_name(&Type::Adt(list)), "List<Int>");
    }

    #[test]
    fn result_error_side_is_generic() {
        // Koka's `error<a>` fixes the error type to `exception`. Ours does not.
        let mut p = Program::new();
        let store_err = p.declare_adt(
            "StoreError",
            vec![Ctor {
                name: "NotFound".into(),
                fields: vec![Type::Str],
            }],
        );
        let r = p.declare_result(Type::Int, Type::Adt(store_err));
        assert_eq!(p.type_name(&Type::Adt(r)), "Result<Int, StoreError>");
        assert_eq!(p.adt(r).ctors[1].fields[0], Type::Adt(store_err));
    }
}
