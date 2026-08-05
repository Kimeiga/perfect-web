/* RQ-2 case B — selective discharge.

   Pre-registered Outcome 4 (hard fail) is: "propagation works but handling
   database.read cannot preserve unrelated effects such as trace".

   `both-effects` requires <database-read, trace-effect>. `discharge-db` handles
   ONLY database-read. The residual row must be exactly <trace-effect> — not
   empty (which would falsely erase trace) and not still containing
   database-read (which would mean the handler did not discharge).
*/
module selective

pub effect database-read
  fun read-store( id : string ) : string

pub effect trace-effect
  fun emit( message : string ) : ()

pub effect secret-read
  fun payment-key() : string

// Requires both effects.
pub fun both-effects( id : string ) : <database-read, trace-effect> string
  emit("loading " ++ id)
  read-store(id)

// Handle ONLY database-read. Residual must be <trace-effect>.
pub fun discharge-db( id : string ) : trace-effect string
  with handler
    fun read-store( sid ) "store:" ++ sid
  both-effects(id)

// Handle ONLY trace. Residual must be <database-read>.
pub fun discharge-trace( id : string ) : database-read string
  with handler
    fun emit( _m ) ()
  both-effects(id)

// Both handled -> total.
pub fun discharge-all( id : string ) : string
  with handler
    fun emit( _m ) ()
  discharge-db(id)

// Effect narrowing through the inferred generic helper from case A.
pub fun my-map( xs, f )
  match xs
    Nil          -> Nil
    Cons(x, rest) -> Cons( f(x), my-map(rest, f) )

// A generic helper must not be able to HIDE a capability. `leaky` passes a
// secret-reading callback through the generic helper; the secret effect must
// still appear in the inferred row.
pub fun leaky( ids : list<string> ) : <secret-read> list<string>
  my-map( ids, fn(_id) payment-key() )

pub fun main() : console ()
  println( discharge-all("s1") )
