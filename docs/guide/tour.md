# A tour of Pleris, part 1: the language

This part covers the language itself: modules, types, functions and
effects. [Part 2](tour-server.md) covers data on the server, and
[part 3](tour-pages.md) pages. [Errors and diagnostics](errors.md) shows
what the compiler says when a program is wrong.

Every example is a whole program, compiled by a test with the real
compiler and the standard packages, as you would check it:

```sh
target/debug/pw check packages/pw-std/*.pw packages/pw-platform-web/*.pw shop.pw
```

## Modules and imports

A file is one module, and says its name first. It imports what it uses.

```pw
module shop.prices

import List
import String

fn total(prices: List<Int>) -> Int !{} {
    List.fold(prices, 0, (sum, price) => sum + price)
}

fn shouted(name: String) -> String !{} {
    String.to_upper(name)
}
```

- `import List` gives a module: you write `List.fold(..)`.
- `import context.{ current_session }` gives names, used bare:
  `current_session()`.
- The types `Int`, `Float`, `String`, `Bool`, `List<T>`, `Option<T>` and
  `Result<T, E>` need no import. The functions on them (`List.fold`,
  `String.length`) do: without `import List`, `List.fold` is refused with
  PW0021.

Two files make one program when you check them together. A module imports
another by its name:

```pw file=shop/money.pw
module shop.money

opaque type Cents = Int

fn cents(n: Int) -> Cents !{} {
    Cents(n)
}

fn added(a: Cents, b: Cents) -> Cents !{} {
    Cents(a.value + b.value)
}

fn display(amount: Cents) -> String !{} {
    let n = amount.value
    let pad = if n % 100 < 10 { "0" } else { "" }
    "${n / 100}.{pad}{n % 100}"
}
```

```pw file=shop/cart.pw
module shop.cart

import shop.money.{ Cents, cents, added, display }

fn total_shown(prices: List<Int>) -> String !{} {
    let mut total = cents(0)
    for p in prices {
        total = added(total, cents(p))
    }
    display(total)
}
```

`shop.cart` can hold a `Cents` and pass it on, but cannot read its `Int`:
`.value` is the declaring module's alone (ADR-0048). Modules may not
import each other in a cycle (PW0025), and a declaration marked `private`
is not importable (PW0023, ADR-0324).

## Values and types

```pw
module shop.values

fn examples() -> Int !{} {
    let count: Int = 5_000_000
    let ratio: Float = 0.25
    let name: String = "Blue Bottle"
    let open: Bool = true
    let greeting = "Welcome to {name}, open: {open}"
    let braces = "a literal \{ and \}"
    if (open & count > 0) | !open {
        count / 3
    } else {
        count % 3
    }
}
```

- `Int` is an exact 64-bit integer. `/` and `%` are Euclidean: `-7 / 2`
  is `-4` and `-7 % 2` is `1` (ADR-0039). There is no implicit conversion
  between `Int` and `Float` (`Float.from_int`).
- A string holds `{expression}` holes. `\{` writes a brace (ADR-0049).
- **`&` is and, `|` is or, `!` is not.** `&&` and `||` do not exist.
  Mixing `&` and `|` in one chain needs parentheses, because neither binds
  tighter (ADR-0322):

```pw fails=PW0031
module shop.logic

fn shown(open: Bool, count: Int) -> Bool !{} {
    open & count > 0 | !open
}
```

## Records

A record type has one constructor, its name, and named fields:

```pw
module shop.items

type Item = Item { name: String, price: Int, sold_out: Bool }

fn espresso() -> Item !{} {
    Item { name: "Espresso", price: 350, sold_out: false }
}

fn discounted(item: Item, off: Int) -> Item !{} {
    Item { name: item.name, price: item.price - off, sold_out: item.sold_out }
}
```

There is no spread or update syntax: a changed record is built again, every
field named. A field read is `item.price`.

## Opaque types and invariants

An opaque type wraps a representation in a new name. Its invariant, after
`where`, is checked wherever a value is made:

```pw
module shop.ids

import String

opaque type StoreId = Int

opaque type Handle = String where String.length(value) >= 1 & String.length(value) <= 15

fn handle(draft: String) -> Option<Handle> !{} {
    if String.length(draft) >= 1 & String.length(draft) <= 15 {
        Some(Handle(draft))
    } else {
        None
    }
}

fn store(n: Int) -> StoreId !{} {
    StoreId(n)
}
```

`Handle(draft)` compiles because the `if` around it shows the invariant
holds (ADR-0179, ADR-0225). Write `Handle(draft)` where nothing shows it,
and the compiler refuses it (PW0622):

```pw fails=PW0622
module shop.handles

import String

opaque type Handle = String where String.length(value) >= 1 & String.length(value) <= 15

fn handle(draft: String) -> Handle !{} {
    Handle(draft)
}
```

Two opaque types over `Int`, a `StoreId` and an `OrderId`, are different
types: a store id cannot be passed where an order id is expected (PW0605).

## Sum types and matching

A sum type lists its cases. A case may carry values:

```pw
module shop.orders

type Status =
    | Placed
    | Preparing(Int)
    | OnTheWay(String)
    | Delivered

fn described(status: Status) -> String !{} {
    match status {
        Placed => "Placed",
        Preparing(minutes) => "Ready in {minutes} min",
        OnTheWay(courier) => "{courier} is on the way",
        Delivered => "Delivered",
    }
}

fn done(status: Status) -> Bool !{} {
    match status {
        Delivered => true,
        _ => false,
    }
}
```

- A `match` handles every case, or the compiler refuses it (PW0305). `_`
  matches the rest.
- A name in a pattern that starts with a capital letter is a case; any
  other name binds. So a misspelt case is refused (PW0608) instead of
  silently matching everything (ADR-0195).
- `Option<T>` (`Some(x)`, `None`) and `Result<T, E>` (`Ok(x)`, `Err(e)`)
  are sum types too. There is no `null`.

## Functions

```pw
module shop.functions

import List

fn first<T>(items: List<T>) -> Option<T> !{} {
    List.get(items, 0)
}

fn applied(n: Int, f: fn(Int) -> Int) -> Int !{} {
    f(n)
}

fn doubled_all(ns: List<Int>) -> List<Int> !{} {
    List.map(ns, (n) => n * 2)
}

fn evens(ns: List<Int>) -> List<Int> !{} {
    List.filter(ns, (n) => n % 2 == 0)
}

fn largest(ns: List<Int>) -> Int !{} {
    let mut best = 0
    for n in ns {
        if n > best {
            best = n
        }
    }
    best
}
```

- A function's last expression is its value. `if` and `match` are
  expressions.
- `fn first<T>` is generic. `fn(Int) -> Int` is a function type, and
  `(n) => n * 2` a lambda.
- `let` binds once; `let mut` may be assigned again.

## Effects

Every function has an *effect row*: what it does besides computing a value.
`!{}` is empty: the function is pure.

```pw
module shop.effects

type Store = Store { name: String }

fn load(id: Int) -> Store !{ database.read<Store> }
    host "shop:data/stores#get"

fn title(id: Int) -> String !{ database.read<Store> } {
    let store = load(id)
    "Welcome to {store.name}"
}
```

- `database.read<Store>` is an effect the web platform declares, with
  where it may run (the origin) and what grants it (ADR-0026).
- `title` calls `load`, so it performs `database.read<Store>` too, and
  says so. Declare `!{}` on it and the compiler refuses it (PW0400).
- A function with no row at all claims nothing, and its effects are
  inferred and passed to its callers.
- A view and a page render with no effect (charter §7.4): they read data
  through declared queries ([part 2](tour-server.md)).

## Results and `?`

A function that can fail returns a `Result`. `?` returns the error early:

```pw
module shop.parse

type ParseError =
    | Empty
    | Negative(Int)

fn quantity(n: Int) -> Result<Int, ParseError> !{} {
    if n < 0 {
        Err(Negative(n))
    } else if n == 0 {
        Err(Empty)
    } else {
        Ok(n)
    }
}

fn doubled(n: Int) -> Result<Int, ParseError> !{} {
    let q = quantity(n)?
    Ok(q * 2)
}
```

## Money

Pleris has no built-in money type: the store writes one, as an opaque type
whose module alone reads its cents, so no other module can round an amount
by accident (ADR-0327). Modelled on `examples/domain.pw`:

```pw
module shop.dollars

type USD = USD { code: String }

opaque type Money<C> = Int

fn cents(n: Int) -> Money<USD> !{} {
    Money(n)
}

fn plus<C>(a: Money<C>, b: Money<C>) -> Money<C> !{} {
    Money(a.value + b.value)
}

fn times<C>(amount: Money<C>, count: Int) -> Money<C> !{} {
    Money(amount.value * count)
}

fn display(price: Money<USD>) -> String !{} {
    let n = price.value
    let pad = if n % 100 < 10 { "0" } else { "" }
    "${n / 100}.{pad}{n % 100}"
}
```

`Money<USD>` and `Money<EUR>` are different types, so they cannot be added.
The store's real module also takes a part of an amount only at a named
`Rate` and `Rounding` (`HalfUp`, `HalfEven`, ..).

Time is in [part 2](tour-server.md#time).
