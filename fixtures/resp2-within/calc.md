# F6 期限ちょうど — 手導出

O は p の Judge.check が Record を直接読む経路 [j]、上限2分。
U は j の fail が out を停止する全経路の証拠。[j,out] の両端込みで2+3=5分。
先行 Reader の1分は j→out の経路外であり加えない。T=max(2,5)=5、within5 なので satisfied。
time_frame の witness は refusal、calls=[j,out]、duration_upper=required_duration=5。
実行の elapsed/work は returned=3、done=6分。静的時間の合否を root safety に fold しない。
故障対は within の5を4へ一回だけ変更する。O/U は両側で全構造一致。
