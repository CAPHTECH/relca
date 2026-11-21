# clock だけを5分へ

7分の経路は未確定。clock の層を閉じた後の最早到達は6分。
Lead 下界は (3×2+3×4+5+6)/8=29/8分。値にはしない。
given reached は未確定が残り truncated、下界無し。
Done も切断葉では到達しないと断定できず、条件付きは truncated。
Done 全流入は5分で確定したreworked経路が1/8あるので、全方策で到達質量1未満。
beyond_reached_mass、下界無し（切断の有無に依らず有限平均にはならない）。
reachable done の証人は残り satisfied/exit 0、bounds.truncated=true。
