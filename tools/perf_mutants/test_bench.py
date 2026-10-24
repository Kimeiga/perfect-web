import copy
import unittest
from bench import parse_summary, validate
from compare import inspect


class MutantSpeedControls(unittest.TestCase):
    def test_summary_is_exactly_one_complete_census(self):
        line = ("mutant programs: 344 mutants of a program's source, 344 judged, "
                "in 2 programs (a, b); 520 checks; 40 refused by the rule "
                "their script names, 0 failing\n")
        obj = parse_summary(line)
        validate(obj,344)
        self.assertEqual(obj["checks"],520)
        with self.assertRaises(ValueError):
            parse_summary(line+line)

    def test_missing_or_false_mutations_are_not_accepted(self):
        line = ("mutant programs: 344 mutants of a program's source, 343 judged, "
                "in 1 programs (a); 519 checks; 40 refused by the rule "
                "their script names, 0 failing\n")
        with self.assertRaises(ValueError):
            validate(parse_summary(line),344)

    def test_comparison_preserves_full_same_head_evidence(self):
        template = {"sha":"a"*40, "workers":6,"wall_seconds":90,
                    "check_exit":0, "summary":{"mutants":344,"judged":344,
                    "programs":1,"roots":["a"],"checks":515,"named":40,"failures":0}}
        records=[]
        for w,t in [(2,80),(4,70),(6,90)]:
            c=copy.deepcopy(template)
            c["workers"],c["wall_seconds"]=w,t
            records.append(c)
        self.assertEqual(inspect(records)[0]["workers"],4)
        with self.assertRaises(ValueError):
            inspect(records[:2])
        bad=copy.deepcopy(records)
        bad[0]["summary"]["checks"]+=1
        with self.assertRaises(ValueError):
            inspect(bad)
