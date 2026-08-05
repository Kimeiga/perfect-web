/* NEGATIVE case B — this module COMPILES, and that is the problem.

   Companion to `negative-nonexhaustive.kk`. That file omits a variant from a
   match in a function declared TOTAL, and Koka rejects it. This file omits a
   variant from a match in a function that declares the `exn` effect, and Koka
   ACCEPTS it: the partial match is modelled as "this function may raise", not as
   "this match is incomplete".

   Consequence (spike finding F-8): Koka does not enforce exhaustiveness as an
   independent static rule. It enforces it only for functions whose effect row
   excludes `exn`. Charter §7.1 ("exhaustive pattern matching") and §16.2
   ("unhandled ADT variant") therefore CANNOT be delegated to Koka wholesale.

   run.sh asserts that this file compiles AND that running it produces a runtime
   pattern-match failure — pinning the gap in place so a future Koka release that
   closes it is noticed.
*/
module negative-nonexhaustive-exn

pub type order-state
  Draft
  Pricing
  Cancelled( reason : string )

// `Cancelled` is not matched. Permitted because the signature declares `exn`.
pub fun describe-order( s : order-state ) : exn string
  match s
    Draft   -> "draft"
    Pricing -> "pricing"

pub fun main() : console ()
  val ok = try({ describe-order(Draft) }, fn(x) "EXN:" ++ x.show)
  println("Draft     -> " ++ ok)
  val bad = try({ describe-order(Cancelled("out of stock")) }, fn(x) "EXN:" ++ x.show)
  println("Cancelled -> " ++ bad)
