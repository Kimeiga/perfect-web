/* NEGATIVE case — this module MUST FAIL to compile.

   Charter §7.1 requires exhaustive pattern matching, and §16.2 lists
   "unhandled ADT variant" as a rejected category. A claim that the compiler
   enforces exhaustiveness is worthless unless a non-exhaustive program is
   demonstrated to fail. run.sh compiles this file and asserts a nonzero exit.

   `Cancelled` is deliberately omitted from the match below.
*/
module negative-nonexhaustive

pub type order-state
  Draft
  Pricing
  Confirmed( confirmation : string )
  Delivered( receipt : string )
  Cancelled( reason : string )

pub fun describe-order( s : order-state ) : string
  match s
    Draft         -> "draft"
    Pricing       -> "pricing"
    Confirmed(c)  -> "confirmed:" ++ c
    Delivered(r)  -> "delivered:" ++ r
