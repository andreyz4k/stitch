<!-- # <img src="dream_egg.png" alt="egg of dreams" height="40" align="left"> DreamEgg -->

# Stitch

# Quickstart

Run `cargo run --release --bin=compress data/cogsci/nuts-bolts.json --compress-max-arity=2 --iterations=3`

In around 10 seconds this should produce an output like

```
=======Compression Summary=======
Found 3 inventions
Cost Improvement: (3.92x better) 1919558 -> 489600
inv0 (1.78x wrt orig): utility: 840320 (final_cost: (1079238,1079238); (1.78x,1.78x)) | uses: 320 | body: [inv0 arity=2: (T (repeat (T l (M 1 0 -0.5 (/ 0.5 (tan (/ pi #0))))) #0 (M 1 (/ (* 2 pi) #0) 0 0)) (M #1 0 0 0))]
inv1 (2.84x wrt orig): utility: 402990 (final_cost: (676248,676248); (1.60x,1.60x)) | uses: 190 | body: [inv1 arity=2: (repeat (T (T #0 (M 0.5 0 0 0)) (M 1 0 (* #1 (cos (/ pi 4))) (* #1 (sin (/ pi 4))))))]
inv2 (3.92x wrt orig): utility: 186648 (final_cost: (489600,489600); (1.38x,1.38x)) | uses: 168 | body: [inv2 arity=2: (#0 (T (T c (M 2 0 0 0)) (M #1 0 0 0)))]
Time: 9175ms
Wrote to "out/out.json"
```

Note that in these inventions `#i` is used for invention variables and `$i` for original program variables.

To see a full list of command line options run `cargo run --release --bin=compress -- --help`
