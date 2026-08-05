/* spike: koka-js-interop — the domain module.

   Charter §14 Milestone 0 task 8 asks this spike to prove three things:
     1. Koka ADTs and effect handlers compile on Apple Silicon;
     2. generated JavaScript can be imported from Node as an ES module;
     3. a `maybe`/`result` value crosses the boundary without unchecked guessing.

   This module is deliberately shaped like the eventual Milestone 1 semantic
   kernel (charter §14 M1 tasks 1-4) so the spike measures the real thing rather
   than a toy: domain ADTs, an opaque-ish id, a declared effect, a handler, and a
   pure function whose effect row must come out empty.
*/
module store

// ---------------------------------------------------------------------------
// Domain values (charter §7.1, §15.1)
// ---------------------------------------------------------------------------

// Koka has no `opaque` keyword; a single-constructor value type is the closest
// available nominal wrapper. Whether that is enough to stop a raw `string` being
// used as a `store-id` is a MILESTONE 1 question, not answered here.
pub value struct store-id
  value : string

pub value struct money-usd
  minor-units : int

pub struct store
  id : string
  name : string

pub struct menu-item
  id : string
  name : string
  price : money-usd
  available : bool

// Charter §7.1 requires `Result<T, E>` with a *generic* error type.
// Koka's built-in `error<a>` fixes the error side to `exception`, and
// `either<a,b>` uses `Left`/`Right` names that carry no domain meaning.
// So the project declares its own. See README finding F-3.
pub value type qresult<a, e>
  QOk( value : a )
  QErr( error : e )

pub type store-error
  StoreNotFound( id : string )
  StoreUnavailable
  StoreDecodeFailed( field : string )

// Charter §15.1 order lifecycle, trimmed. Exists to prove that a multi-variant
// ADT with mixed payload/payload-free constructors survives the JS boundary.
pub type order-state
  Draft
  Pricing
  Confirmed( confirmation : string )
  Delivered( receipt : string )
  Cancelled( reason : string )

// ---------------------------------------------------------------------------
// Effects (charter §7.2)
// ---------------------------------------------------------------------------

// `database.read<Stores>` from the charter's core effect families.
pub effect database-read
  fun read-store( id : string ) : maybe<store>
  fun read-menu( id : string ) : list<menu-item>

// `trace` from the charter's core effect families.
pub effect trace-effect
  fun emit( message : string ) : ()

// ---------------------------------------------------------------------------
// Pure computation — the effect row must come out EMPTY.
// ---------------------------------------------------------------------------

// Charter §7.2: `calculate_subtotal : Cart -> Money<USD> !{}`.
// The `.kki` interface file is checked by kki-extract.mjs to confirm that no
// effect row is inferred here. That check is the actual deliverable, not this
// function's arithmetic.
pub fun calculate-subtotal( items : list<menu-item> ) : money-usd
  Money-usd( items.foldl( 0, fn(acc, it) acc + it.price.minor-units ) )

pub fun available-items( items : list<menu-item> ) : list<menu-item>
  items.filter( fn(it) it.available )

// Exhaustive match over an ADT. Removing a variant from `order-state` must make
// this fail to compile — that is the charter §7.1 "exhaustive pattern matching"
// property, verified by the negative case in run.sh.
pub fun describe-order( s : order-state ) : string
  match s
    Draft              -> "draft"
    Pricing            -> "pricing"
    Confirmed(c)       -> "confirmed:" ++ c
    Delivered(r)       -> "delivered:" ++ r
    Cancelled(reason)  -> "cancelled:" ++ reason

// ---------------------------------------------------------------------------
// Effectful computation — the effect row must MENTION the effects.
// ---------------------------------------------------------------------------

// Charter §7.2: `load_store : StoreId -> Result<Store, StoreError> !{ database.read<Stores>, trace }`
pub fun load-store( id : string ) : <database-read, trace-effect> qresult<store, store-error>
  emit("load-store:" ++ id)
  match read-store(id)
    Just(s)  -> QOk(s)
    Nothing  -> QErr(StoreNotFound(id))

pub fun load-store-subtotal( id : string ) : <database-read, trace-effect> qresult<money-usd, store-error>
  match load-store(id)
    QErr(e)  -> QErr(e)
    QOk(_s)  ->
      emit("load-menu:" ++ id)
      QOk( calculate-subtotal( read-menu(id).available-items ) )

// ---------------------------------------------------------------------------
// Handlers — local development interpretations (charter §14 M1 task 3)
// ---------------------------------------------------------------------------

// An in-memory fixture database. The charter's M1 task 3 wants
// `database -> in-memory map or SQLite adapter boundary`; this is the in-memory
// half, deliberately deterministic so tests have no clock or randomness.
// NOTE (finding F-6): a multi-operation effect must be discharged by ONE
// `handler` block. The `with fun op(..)` shorthand installs a handler for the
// whole effect while defining only one operation, so chaining two of them leaves
// the second operation unhandled: "operator read-menu is not handled".
pub fun fixture-db( action : () -> <database-read|e> a ) : e a
  with handler
    fun read-store( id )
      if id == "store_47" then Just( Store("store_47", "Blue Bottle") )
      elif id == "store_48" then Just( Store("store_48", "Sightglass") )
      else Nothing
    fun read-menu( id )
      if id == "store_47" then
        [ Menu-item("item_1", "Espresso",   Money-usd(350), True),
          Menu-item("item_2", "Cortado",    Money-usd(475), True),
          Menu-item("item_3", "Cold Brew",  Money-usd(525), False) ]
      else []
  action()

// A `trace` handler that discards. The collecting variant lives in Milestone 1;
// this spike only needs the effect to be *handleable*.
pub fun silent-trace( action : () -> <trace-effect|e> a ) : e a
  with fun emit( _message ) ()
  action()

// ---------------------------------------------------------------------------
// The JS-facing surface.
// ---------------------------------------------------------------------------

// Effects must be fully handled before a value can be handed to JavaScript.
// This function's inferred effect row is empty, and kki-extract.mjs asserts
// that — which is the point. An unhandled effect would be a suspended
// computation, not a value, and could not cross the boundary at all.
pub fun load-store-for-js( id : string ) : qresult<store, store-error>
  with silent-trace
  with fixture-db
  load-store(id)

pub fun load-subtotal-for-js( id : string ) : qresult<money-usd, store-error>
  with silent-trace
  with fixture-db
  load-store-subtotal(id)

pub fun sample-order-states() : list<order-state>
  [ Draft, Pricing, Confirmed("conf_1"), Delivered("rcpt_1"), Cancelled("out of stock") ]

pub fun show-store-result( r : qresult<store, store-error> ) : string
  match r
    QOk(s)                     -> "QOk(" ++ s.name ++ ")"
    QErr(StoreNotFound(id))    -> "QErr(StoreNotFound:" ++ id ++ ")"
    QErr(StoreUnavailable)     -> "QErr(StoreUnavailable)"
    QErr(StoreDecodeFailed(f)) -> "QErr(StoreDecodeFailed:" ++ f ++ ")"

pub fun main() : console ()
  println( "load-store-for-js(store_47) -> " ++ show-store-result(load-store-for-js("store_47")) )
  println( "load-store-for-js(store_99) -> " ++ show-store-result(load-store-for-js("store_99")) )
  val descriptions = sample-order-states().map(describe-order)
  println( "sample-order-states -> " ++ descriptions.join(", ") )
