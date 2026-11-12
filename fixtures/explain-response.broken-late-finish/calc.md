# X-R: first deadline violation, not eventual completion

Only change fixed4 -> fixed6. initial0 received triggers deadline5;
queue1/start2 at0; advance3 to6 establishes 5 < 6. Count3, time6,
trigger_step0, deadline5. Do not append finish4/join5/end6 to this prefix.
No alternative branch or tie. start carries Worker.run/x and null result;
advance has process location and resource null, not a fake completed result.
Call line5 col5 byte181; clause line11. Expected root violated/1,
complete/model, replay verified and diagnostics [] (no Cut warning).
