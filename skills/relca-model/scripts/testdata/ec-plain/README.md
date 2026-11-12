# ec-plain(test_review_page.py の ResponseRetryPageTest の入力)

- `model.relca`: 電子商取引の注文と決済の試行の plain 案(idempotency key なしの決済、`response_loss nondet retry max_attempts 2`)。sha256 `ee84bd699bf8e246e9da8a0523f0f50ce0d884c322cfcaf342244c707168e004`。
- `explain.json`: 同じモデルの `relca explain` の出力(sha256 `7f8e9b1934e71a64b20529128cd854787701b6fe298e4cf5c92dff17918b2c0f`、verdict violated、exit 1)から、次だけを抜いたもの。値は書き換えていない。
  - 反例 `contract PayControls #2 at_most_once` の command のある段(27・28・35・40・41・48)。`values` と events の `location` は落とした。
  - `properties` のうち同じ性質と `receipt_resolution`、`assumptions` のうち `response.*` と `agency.*`、`agency.unchecked`。
  - `response_receipts`・`response_analysis`・`case_outcomes`・`terminals`・`bounds`・`cut` の一部・`model`・`ledger` の process の選択。

抜き出したので、これは relca の出力そのものではない。fold や Agency の他の欄の検査には使わない。
