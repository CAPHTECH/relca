# ADR0053: 実行前の手計算

模型は架空。分/JPY。新fixtureは0。閉じた入力はsupport/deferred_metrics_inputs.rs。

19 named construction/18 distinct DSL source。unrelated/unusedだけが同じsourceを共用する。
解決済みで指標とdeferのある15件と、defer無し2件・指標無し1件・静的拒否1件を区別する。
公開IRのActorIdを壊す一故障は同じactor入力から作り、別のDSL入力として水増ししない。

A.xは3分、pが実行、R.yは2分、qが実行、R.zは4分。逐次のx/yは[0,3),[3,5)。
単価はA10円/分、R20円/分、x1円/開始、y2円/開始、z4円/開始。
deferなし: work5、people5、occupancy30+40=70、start1+2=3、total73、lead5。
xのみはwork3、occupancy30、start1、total31、lead3。x/y/zはwork9、occupancy150、start7、total157、lead9。

Done=reached(end.done)、Expected/Arrivals=1、P95=5(または3/9)。固定時間でsimulation3反復は全て同じ値、通常Sampleのsamples3。成功率は点1。標本推定を証明と呼ばない。

defer RはTなので、RだけでなくAのwork3/費用30とxの開始費1もnot_configuredにする。
defer R.yもTなので、同Actor zのTask.work4と開始費4を含め全ての時間費用を保留する。
これは再レビューの裁定による旧期待の意図的な変更で、精密な依存閉包は入れない。
Rを呼ばないprocessではAと全体の3分/31円と品質1/P953を保つ。死枝のR.yは字句callなので依存する。
opaqueラベルは依存を否定できず全Workload/qualityが保留。自然言語の理由を無関係と書いても値は戻らない。

Scalarの保留はRangeならmin/max null、Sampleならmean/min/max null、samples0、reason not_configured。
品質の区間はlower/upper/basis null、reason not_configured。quantile lower_bound null。
既存のtruncated/aborted/never_created/no_terminal等は優先。構造の到着/未完了/終了件数とCore終端のelapsed/work、verifyは変えない。

正の対はdefer無しで全部数値。壊した対は同じ文字列のdefer一つの追加/削除/対象変更だけ。
oracleの負対照はdefer無しA.workの一文字変更、not_configuredの一文字変更、checked移管漏れを拒否する。

pをA/Rで共有した対は、逐次なのでCoreの時間5分は変わらず、pの合計workは保留、
Aのworkも未使用qのworkも保留する。Actor名/粒度ラベルでこの写像を変えない。
hardをRate>=1/10に置く。非依存案は3反復全hit、confidence1/2と2案のBonferroniで
CP下端(1/8)^(1/3)=1/2なのでsatisfied。依存案はnot_configuredなのでhardはinconclusive。
保持したhitsから判定を復活させない。目的だけならverifyとexitを変えない。

既存ADR0052のinvoice通常品質の手値93/125000・231/125000・81/12500は、defer行だけを除いた明示の対照模型で引き続き検査する。元のinvoiceファイルは変更しない。元のCutを保持した入力はADR0053でnot_configuredを要求する。Ruleを理由に撤回するのではない。
