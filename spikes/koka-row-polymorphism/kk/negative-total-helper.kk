/* RQ-2 negative case — MUST FAIL to compile.

   `strict-map` explicitly annotates its callback as TOTAL (no effect row).
   Passing a database-reading callback must be rejected. If this compiled, the
   effect system could be bypassed simply by annotating a helper as pure, and
   every positive result above would be worthless.
*/
module negative-total-helper

pub effect database-read
  fun read-store( id : string ) : string

// Callback declared TOTAL: `(a) -> b`, no effect row.
pub fun strict-map( xs : list<a>, f : (a) -> b ) : list<b>
  match xs
    Nil          -> Nil
    Cons(x, rest) -> Cons( f(x), strict-map(rest, f) )

// ERROR: the callback performs database-read but strict-map demands a total one.
pub fun bad( ids : list<string> ) : database-read list<string>
  strict-map( ids, fn(id) read-store(id) )
