# A tour of Pleris, part 3: pages

[Part 1](tour.md) covered the language and [part 2](tour-server.md) data
on the server. This part puts them on pages: routes, markup, state in the
browser, forms, layouts and navigation.

Markup is HTML with blocks, as in Svelte. A page is rendered on the server
as plain HTML, readable with JavaScript off, and then *resumed* in the
browser: the browser attaches the page's handlers without running the page
again (charter §8.5).

## Pages and routes

```pw
module shop.pages

opaque type StoreId = String

page Home() {
    route "/"

    view {
        <title>Stores</title>
        <main>
            <h1>Stores</h1>
            <a href="/stores/blue-bottle">Blue Bottle</a>
        </main>
    }
}

page StorePage(id: StoreId) {
    route "/stores/{id}"

    view {
        <title>Store</title>
        <main>
            <h1>Store {id}</h1>
            <a href="/">All stores</a>
        </main>
    }
}
```

- A page's parameters come from its route: `{id}` names `id`. A route
  names each parameter once (PW0340), and a path parameter is text: a
  `String`, or an opaque type over one (PW0621).
- Every link is matched against the program's routes (PW5009).
- A page with a route has a `<title>`, first in its view (PW5029).

**Query parameters** are declared in the route, after `?`, and are optional
by type (ADR-0309):

```pw
module shop.search

page Search(q: Option<String>) {
    route "/search{?q}"
    cache private

    view {
        <title>Search</title>
        <main>
            <form method="get" action="/search">
                <label>Find <input type="search" name="q" /></label>
                <button>Search</button>
            </form>
            {#match q}
                {:Some(text)}
                    <p>You searched for {text}.</p>
                {:None}
                    <p>Nothing searched yet.</p>
            {/match}
        </main>
    }
}
```

A link or a `GET` form may give only the parameters the route declares
(PW5050).

## Markup blocks

```pw
module shop.menu

type Item = Item { id: String, name: String, price: Int, sold_out: Bool }

type Panel =
    | Shut
    | Menu
    | Help

view Row(item: Item) {
    <li>
        <span>{item.name}</span>
        {#if item.sold_out}
            <span class="sold-out">Sold out</span>
        {:else}
            <span>{item.price} cents</span>
        {/if}
    </li>
}

page MenuPage() {
    route "/menu"
    cache private

    signal panel: Panel = Panel.Menu
    signal items: List<Item> = [Item { id: "1", name: "Espresso", price: 350, sold_out: false }]

    view {
        <title>Menu</title>
        <main>
            {#match panel}
                {:Shut}
                    <p>Closed.</p>
                {:Menu}
                    <ul>
                        {#each items as item (item.id)}
                            <Row item={item} />
                        {/each}
                    </ul>
                {:Help}
                    <p>Ask at the counter.</p>
            {/match}
        </main>
    }
}
```

- `{#if c} .. {:else} .. {/if}`: `c` is a `Bool`, or a list or a string
  tested for being non-empty. A number is not a condition (PW0609): write
  `count > 0`.
- `{#each items as item (item.id)}`: every list is keyed by a field of its
  rows (PW5011), so a row keeps its place as rows come and go. There is no
  index variable (PW0016). The list is read by path (a signal, a query's
  result, a field): a computed list is [a gap](#where-pw-check-stops).
- `{#match v} {:Case} .. {/match}`: one arm a case, every case handled.
- `view Row(item: Item)`: a view takes props and renders markup. It is
  used as an element, `<Row item={item} />`, and composed in place.

## State in the browser: signals and handlers

```pw
module shop.counter

page Counter() {
    route "/counter"
    cache private

    signal count: Int = 0

    view {
        <title>Counter</title>
        <main>
            <p>Pressed {count} times.</p>
            <button type="button" on:press={() => count = count + 1}>Press</button>
            <button type="button" on:press={() => count = 0}>Reset</button>
        </main>
    }
}
```

- `signal count: Int = 0` is the page's own state. Only a handler changes
  it (PW5300), and the parts of the page that read it are rendered again.
- `on:press` is a click, a tap or Enter on the button. The events are
  `press`, `submit`, `input`, `keydown`, `change` and `close`; `on:click`
  does not exist:

```pw fails=PW5022
module shop.click

page Clicker() {
    route "/clicker"
    cache private

    signal count: Int = 0

    view {
        <title>Clicker</title>
        <button type="button" on:click={() => count = count + 1}>Press</button>
    }
}
```

A handler that calls a command matches on its answer, as the
[tutorial](tutorial.md) shows: `match post(text) { Ok(_) => draft = "",
Err(_) => () }`. The answer says whether the command committed; the new
data comes through the page's queries.

A signal can be read in a part of the page, but not in a `<title>`, and a
part cannot compute a value from two signals, as in `{a + b}`: `pw build`
refuses both, and `pw check` passes them
([where `pw check` stops](#where-pw-check-stops)).

## Forms and `bind:value`

```pw
module shop.note

import String

page Note() {
    route "/note"
    cache private

    signal note: String = ""

    view {
        <title>Note</title>
        <main>
            <label>Your note <textarea bind:value={note}></textarea></label>
            <p>{280 - String.length(note)} characters left</p>
            <button type="button" disabled={String.length(note) == 0} on:press={() => note = ""}>Clear</button>
        </main>
    }
}
```

- `bind:value={note}` keeps a `String` signal and an `<input>` or a
  `<textarea>` equal. It binds strings only: there is no checkbox binding
  yet, and a `<select>` marks its `selected` option instead (PW5036).
- Every control has a label (PW5014).
- A form that changes data submits through a handler,
  `<form on:submit|prevent={() => ..}>`. A plain `<form method action>`
  must go where something answers that method (PW5041): a page answers
  `GET`.

## Reading data on a page

A page reads queries with `let`. A query that answers a `Result` gives its
`Ok` value, and `not_found_on` names the case that means the address names
nothing, answered 404 (ADR-0163); any other error is a 503:

```pw
module feed.thread

opaque type PostId = String

type Post = Post { id: PostId, text: String, likes: Int }

type FeedError =
    | Unavailable
    | NotFound

fn thread(id: PostId) -> Result<Post, FeedError> !{ database.read<Post> }
    host "feed:data/posts#thread"

public query Thread(id: PostId) -> Result<Post, FeedError>
    freshness   5.seconds
    consistency snapshot
    cache       shared
    key         id
{
    thread(id)
}

page PostPage(id: PostId) {
    route        "/post/{id}"
    not_found_on FeedError.NotFound
    placement    origin
    cache        private

    let post = query Thread(id)

    view {
        <title>Post</title>
        <main>
            <p>{post.text}</p>
            <p>{post.likes} likes</p>
        </main>
    }
}
```

`session page` and `user page` say whose data a page shows, as a query
does (part 2). A page that asks a predicate of its reader
(`{#if SignedIn}`) is not the same for everyone, so it cannot be public
(PW5049).

## Layouts and navigation

A layout is the markup pages share, with a `<slot />` where each page goes
(ADR-0303):

```pw
module site.layout

layout Shell {
    view {
        <header>
            <nav aria-label="Site">
                <a href="/">Home</a>
                <a href="/about">About</a>
            </nav>
        </header>
        <slot />
    }
}

page Home() {
    route  "/"
    layout Shell

    view {
        <title>Home</title>
        <main>
            <h1>Home</h1>
        </main>
    }
}

page About() {
    route  "/about"
    layout Shell

    view {
        <title>About</title>
        <main>
            <h1>About</h1>
        </main>
    }
}
```

- A link between two pages of one layout keeps the layout in place: the
  browser fetches the next page, swaps the slot, moves the focus to its
  heading, and keeps the history (ADR-0306). Any other link is a normal
  page load.
- A handler goes to a page after its command commits:
  `Ok(_) => navigate OrderPage()` (ADR-0280). `navigate` is only allowed
  there (PW5043).
- A layout has one slot (PW5044) and no parameters (PW0353). Layouts do
  not nest yet.

## Where `pw check` stops

`pw check` runs the compiler's checks. `pw build` goes on to the page's
templates, and refuses some programs `pw check` passes. So a clean check is
not yet a program that runs: build it, as the [tutorial](tutorial.md) does.
`pw check` and `pw build` both report PW6601 for programs the executable backends cannot support.

Each program below is refused by the shared executable-program validation. The guide test requires exactly PW6601 for each one.

A `<title>` cannot read a signal, because the browser does not render the
title again (ADR-0137):

```pw fails=PW6601
module shop.titled

page Counter() {
    route "/counter"
    cache private

    signal count: Int = 0

    view {
        <title>Pressed {count}</title>
        <button type="button" on:press={() => count = count + 1}>Press</button>
    }
}
```


A part of a page cannot compute a value from two signals (ruling 0073-a):

```pw fails=PW6601
module shop.sum

page Sum() {
    route "/sum"
    cache private

    signal a: Int = 1
    signal b: Int = 2

    view {
        <title>Sum</title>
        <main>
            <p>{a + b}</p>
            <button type="button" on:press={() => a = a + 1}>Add to a</button>
            <button type="button" on:press={() => b = b + 1}>Add to b</button>
        </main>
    }
}
```


An `{#each}` reads its list by path, and a call in its head computes the
list, so check and build refuse it (PW6601). This pure example isolates the executable support limit:

```pw fails=PW6601
module shop.head

type Item = Item { name: String }

fn items() -> List<Item> !{} { [Item("Coffee")] }

view Menu() !{} {
    <ul>
        {#each items() as item (item.name)}
            <li>{item.name}</li>
        {/each}
    </ul>
}
```


And an `<img>` with no `alt` is not refused, though an `<input>` with no
label is (PW5014):

```pw gap=check
module shop.logo

page Home() {
    route "/"

    view {
        <title>Shop</title>
        <main>
            <img src="/logo.png" width="64" height="64">
            <h1>Shop</h1>
        </main>
    }
}
```
