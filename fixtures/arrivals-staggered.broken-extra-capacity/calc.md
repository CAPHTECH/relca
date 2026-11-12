# F2 単一故障: Actor capacity 1 → 2

占有区間はcase1[0,3)、case2[1,4)、case3[3,6)。待ちは0,0,1。
completed_at=(3,4,6)、elapsed=(3,3,4)、work=(3,3,3)、result_at null。
最小=最大。確率未入力、全案件done、safety/reachable各satisfied、拒否0、切断無し。
容量を案件ごとに複製すればcase3が5に終わるため、それとは異なる期待。
