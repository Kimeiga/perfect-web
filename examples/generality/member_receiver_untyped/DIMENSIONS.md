# member_receiver_untyped challenge dimensions

An unconstrained lambda parameter has no receiver type: even reading .name
cannot be checked for effects or privacy. The typed-neighbour writes the
parameter type and reads a real field. The generic-effects suite also checks
an explicitly typed effectful member, generic result/record substitution,
ordinary loop/match binders and an unreachable Any-valued receiver.
