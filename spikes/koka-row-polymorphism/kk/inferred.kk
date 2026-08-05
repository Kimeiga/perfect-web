/* RQ-2 case A — does Koka INFER effect polymorphism for a user-defined generic
   helper, with no annotation at all?

   This is the difference between pre-registered Outcome 1 (automatic
   propagation, clean pass) and Outcome 2 (annotation required).

   `my-map` carries NO signature. If Koka infers an effect-polymorphic type,
   passing an effectful callback must make the caller's row mention the effect.
*/
module inferred

pub effect database-read
  fun read-store( id : string ) : string

pub effect trace-effect
  fun emit( message : string ) : ()

// No type signature at all. What does Koka infer?
pub fun my-map( xs, f )
  match xs
    Nil          -> Nil
    Cons(x, rest) -> Cons( f(x), my-map(rest, f) )

// Layer 2: another un-annotated generic helper over the first.
pub fun transform-items( xs, f )
  my-map( xs, f )

// Layer 3: the caller. Its inferred row is the thing under test.
// If the database-read effect vanishes here, that is Outcome 3 (hard fail).
pub fun page-loader( ids : list<string> ) : database-read list<string>
  transform-items( ids, fn(id) read-store(id) )

// Control: a pure callback through the same helpers must stay pure.
pub fun pure-loader( xs : list<int> ) : list<int>
  transform-items( xs, fn(x) x + 1 )

pub fun fixture-db( action : () -> <database-read|e> a ) : e a
  with handler
    fun read-store( id ) "store:" ++ id
  action()

pub fun main() : console ()
  println( pure-loader([1,2,3]).map(show).join(",") )
  val names = fixture-db( fn() page-loader(["a","b"]) )
  println( names.join(",") )
