//! Manual check against a running ACC: `cargo test --test acc_live -- --ignored --nocapture`
#[test]
#[ignore]
fn reads_live_shared_memory() {
    use race_engineer_ai_lib::games::acc::shm::*;
    let st = MappedPage::<SPageFileStatic>::open(STATIC_NAME).expect("ACC not running");
    let gf = MappedPage::<SPageFileGraphic>::open(GRAPHICS_NAME).expect("graphics");
    let ph = MappedPage::<SPageFilePhysics>::open(PHYSICS_NAME).expect("physics");
    let s = st.read();
    let g = gf.read();
    let p = ph.read();
    let (max_fuel, sectors, pws, pwe, online) = (s.maxFuel, s.sectorCount, s.PitWindowStart, s.PitWindowEnd, s.isOnline);
    println!("sm={} ac={} track={} car={} player={} {}", wstr(&s.smVersion), wstr(&s.acVersion), wstr(&s.track), wstr(&s.carModel), wstr(&s.playerName), wstr(&s.playerSurname));
    println!("maxFuel={max_fuel} sectors={sectors} pitWindow={pws}..{pwe} online={online} dry={} wet={}", wstr(&s.dryTyresName), wstr(&s.wetTyresName));
    let (status, session, laps, pos, active, pid, fuel_x, gap_a, gap_b, rain, grip) = (g.status, g.session, g.completedLaps, g.position, g.activeCars, g.playerCarID, g.fuelXLap, g.gapAhead, g.gapBehind, g.rainIntensity, g.trackGripStatus);
    println!("status={status} session={session} laps={laps} pos={pos} activeCars={active} playerId={pid} fuelXLap={fuel_x} gapAhead={gap_a} gapBehind={gap_b} rain={rain} grip={grip} tyre={}", wstr(&g.tyreCompound));
    let (fuel, speed, gear, road, air) = (p.fuel, p.speedKmh, p.gear, p.roadTemp, p.airTemp);
    println!("fuel={fuel} speed={speed} gear={gear} road={road} air={air}");
}
