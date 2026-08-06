/* Hand-written driver. It exercises the GENERATED module and prints values a
   test can compare against expectations computed independently. */
module main

import store_pricing

pub fun main() : console ()
  val line = Cart_line( 3, 250 )
  println( "line_total=" ++ line_total( line ).show )
  println( "with_delivery_near=" ++ with_delivery( 750, 1000 ).show )
  println( "with_delivery_far=" ++ with_delivery( 750, 9000 ).show )
  println( "label_draft=" ++ label( Draft ) )
  println( "label_confirmed=" ++ label( Confirmed( "ORD-42" ) ) )
