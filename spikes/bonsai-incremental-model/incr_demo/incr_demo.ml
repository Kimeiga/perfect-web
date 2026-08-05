(* spike: bonsai-incremental-model — the Incremental half.

   Charter §14 Milestone 0 task 12 asks this spike to
     "demonstrate that an unrelated state update does not recompute an
      instrumented expensive derived value"
   and to
     "confirm from source which portions use Jane Street Incremental and which
      portions still produce a virtual DOM and diff/patch it".

   Incremental is the mechanism underneath Bonsai's incrementality, so it is
   tested directly and in isolation. The instrumentation is a plain counter
   incremented inside the expensive node — if the node re-fires, the counter
   moves, and no amount of framework claim can hide it.

   Charter §5 lists what to borrow from Incremental: "a stable dependency DAG,
   cutoffs, stabilization, and incremental recomputation of arbitrary derived
   values" — and what NOT to assume: "that its OCaml implementation should
   become the permanent cross-target runtime". *)

open! Core
module Incr = Incremental.Make ()

let expensive_evaluations = ref 0
let cheap_evaluations = ref 0

let () =
  (* Two independent inputs. `store_id` feeds the expensive derived value;
     `cart_count` does not. This mirrors the charter's store page: a cart change
     must not recompute the menu. *)
  let store_id = Incr.Var.create 47 in
  let cart_count = Incr.Var.create 0 in

  (* The instrumented expensive derived value. Depends ONLY on store_id. *)
  let expensive_menu =
    Incr.map (Incr.Var.watch store_id) ~f:(fun id ->
      incr expensive_evaluations;
      (* stand-in for real work *)
      let sum = ref 0 in
      for i = 1 to 200_000 do sum := !sum + (i mod (id + 1)) done;
      !sum)
  in

  let cheap_badge =
    Incr.map (Incr.Var.watch cart_count) ~f:(fun n ->
      incr cheap_evaluations;
      Printf.sprintf "%d items" n)
  in

  let page = Incr.map2 expensive_menu cheap_badge ~f:(fun menu badge ->
    Printf.sprintf "menu=%d badge=%s" menu badge)
  in
  let obs = Incr.observe page in

  let report label =
    Incr.stabilize ();
    Printf.printf "  %-38s expensive=%d cheap=%d\n%!"
      label !expensive_evaluations !cheap_evaluations
  in

  print_endline "=== Incremental: does an unrelated update recompute the expensive node? ===";
  print_endline "";
  report "initial stabilize";
  let after_initial = !expensive_evaluations in

  (* THE TEST. Change only the unrelated input, repeatedly. *)
  for i = 1 to 5 do
    Incr.Var.set cart_count i;
    report (Printf.sprintf "set cart_count := %d (unrelated)" i)
  done;
  let after_unrelated = !expensive_evaluations in

  (* Now change the input the expensive node actually depends on. *)
  Incr.Var.set store_id 48;
  report "set store_id := 48 (related)";
  let after_related = !expensive_evaluations in

  (* Set the SAME value again — Incremental's cutoff should suppress this. *)
  Incr.Var.set store_id 48;
  report "set store_id := 48 again (cutoff)";
  let after_cutoff = !expensive_evaluations in

  print_endline "";
  Printf.printf "  value = %s\n" (Incr.Observer.value_exn obs);
  print_endline "";
  print_endline "=== results ===";
  Printf.printf "  check:unrelated-update-does-not-recompute=%s  (%d -> %d across 5 unrelated updates)\n"
    (if after_unrelated = after_initial then "pass" else "fail")
    after_initial after_unrelated;
  Printf.printf "  check:related-update-does-recompute=%s        (%d -> %d)\n"
    (if after_related > after_unrelated then "pass" else "fail")
    after_unrelated after_related;
  Printf.printf "  check:same-value-write-is-cut-off=%s          (%d -> %d)\n"
    (if after_cutoff = after_related then "pass" else "fail")
    after_related after_cutoff;
  print_endline "";
  print_endline "  Charter §9.5: 'A rerender or local reactive recomputation must not create";
  print_endline "  a new request unless the semantic resource key or policy changed.'";
  print_endline "  Incremental enforces exactly that shape for pure derived values."
