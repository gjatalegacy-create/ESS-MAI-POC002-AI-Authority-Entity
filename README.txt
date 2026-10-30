POC002 — Entiteti AUTORITETI AI: hierarkia ekzekutive native
Bledar Gjata — propozim paradigme nga ESS-MAI
Kapsulë kërkimore për shqyrtim, 30 tetor 2026. Shih README.md për statusin e plotë të provave.

OBJEKTI

Ky POC shqyrton autoritetin si kompetencë ekzekutive brenda një
hierarkie kushtetuese: ka detyrë, juridiksion dhe kushte të zbatueshme;
mund të koordinojë autoritete vartëse, secila me kompetencat e veta.
GCL është kushtetuta, jo një autoritet shtesë që voton.

Shkëputja ruan hierarkinë ekzistuese në AIGovern-Cyber:

GCL / faza Reasoning
  QUANTUM, me rendin e materializuar në UCL
    ASHT: detyra Historical dhe detyra KiE, me scope të dalluar
    PRO: operatorët origjinalë; matja shkencore KiS
    NPRO: operatorët negativë; matja shkencore KiC

ASHT lidh materialin dhe direktivën që konsumohen më tej. QUANTUM
i thërret PRO/NPRO në caller-in e vet; ASHT nuk i thërret drejtpërdrejt.
NPRO dhe endpoint-et KiS/KiC kontrollojnë direktivën përkatëse.
PRO origjinal merr fragmentet e kufizuara, scaffold-in dhe Vault-in;
nuk ka të njëjtin parametër direktive si endpoint-i KiS.

Dy thirrje me kompetenca, origjina evidence ose detyra të ndryshme nuk
janë automatikisht dy ekzekutime të së njëjtës punë. Dështimi i një
rrjedhe nuk i jep kompetenca të reja tjetrës. Vazhdimi kërkon ende
ligjin, juridiksionin, evidencën dhe kapacitetin e vet.

ÇFARË ËSHTË MARRË

Native/runtime përmban LIGHT, QUANTUM, SHADOW dhe varësitë e kompilimit,
pa ndryshuar bajtet e skedarëve të burimit ose testet ekzistuese.
Lista dhe SHA-256 janë në provenance/SOURCE_MANIFEST.json.
Kodi dhe emërtimet origjinale janë ruajtur, edhe aty ku komentet
historike nuk janë përkufizim i plotë i sjelljes aktuale.

Nuk është shtuar një entity wrapper, regjistër i ri autoritetesh,
ligj, prag, rezultat i detyruar apo enum i ri HOLD/FAIL-CLOSED.
Workspace-i dhe Cargo.lock tani janë identikë me burimin, bashkë me
719 skedarët native të manifestuar. Faza fillestare vetëm-QUANTUM
përdorte një workspace më të vogël; plotësimi ka rikthyer manifestin
dhe lockfile-in origjinal, pa ndryshuar asnjë version varësie.

Native përmban më shumë se blloqet e vogla të hierarkisë: kjo është
mbyllja e varësive të crate-it ekzistues, jo pretendim se çdo modul
është thelb i paradigmës. Është përfshirë i gjithë kodi native i runtime-it dhe varësitë e tij.
Nuk përfshihen launcher-at e produktit, build-et ose memoria private.

LIGHT dhe procesi SHADOW/Vault tashmë janë përfshirë dhe ekzekutuar.
run_continuity.py është vetëm drejtues prove: jep pesë hyrje të
shprehura dhe përdor CLI-të native; nuk krijon autoritet, kandidat,
receipt ose verdikt. Memoria nis bosh dhe është e izoluar nga produkti.
Evidenca: pesë hyrje në një çështje, tri HOLD operacionale dhe dy
mbyllje të plota native. Nuk i quajmë pesë mbyllje të plota.

LEXIMI I SAKTË I REZULTATEVE

1. UCL lëshon Historical dhe KiE në një tranzicion superior.
   Rilëshimi në të njëjtën instancë refuzohet. ASHT kontrollon ligjin
   dhe scope-in, bën kërkesën dhe përpunon materialin relevant.

2. Caller-i vazhdon KiE pas dështimit historik vetëm nëse mungesa
   riprodhohet nga request/response real dhe arsyeja përputhet.
   Korruptimi, scope-i i gabuar ose transporti i dështuar nuk trajtohen
   si mungesë kandidati që lejon këtë vazhdim.

3. PRO origjinal mund të mos prodhojë kandidatë/operatorë, ndërsa
   KiS/KiC shqyrtohen me scope dhe lejen e tyre ekzistuese. Matja
   shkencore nuk rihap maskën e operatorëve origjinalë të ndaluar.

4. Mungesa e kapacitetit te endpoint-i KiE është Ok(None), që
   përmbledhja e shënon Held. Një predikat i ekzekutuar me zero është
   tjetër gjë. Mungesa historike shfaqet si Err në ASHT; rruga PD/iZ
   e deklaron në nivelin e saj operacional. PRO vetë nuk ka enum HOLD.

5. UCL dhe gjurma kontrollojnë rendin dhe përsëritjen e fazave në
   rrugën native. Kjo nuk është provë se çdo API publike e thirrur
   arbitrarisht nuk mund të llogarisë të njëjtën gjë përsëri. Nuk
   është shtuar një garanci globale exactly-once për ta bërë provën
   të duket më e fortë. Një cikël i ri nuk ndalohet nga suksesi i
   ciklit të mëparshëm.

6. SHADOW zgjidh riprovimin ekzakt nga memoria e vet përpara gjykimit
   të ri. Prova reale rikthen të njëjtat bajte të përgjigjeve T1/T2,
   pa ndryshuar Vault-in; identiteti me hash tjetër kërkese refuzohet.
   Ky është kufi i protokollit dhe efektit të qëndrueshëm, jo ndalim
   i çdo rillogaritjeje arbitrare jashtë rrugës kanonike.

PROVA ME PESË HYRJE

  1. Pyetje e re; pa kandidatë në Vault -> HOLD / NoVerdict.
  2. Përgjigje pa provën e munguar -> HOLD; ruhet çështja dhe gjurma.
  3. Pohim numerik brenda era00 -> KiS, gjykim native (1,1), T1/T2.
  4. Pyetje e re mbi të njëjtën çështje -> HOLD i lidhur me T2 e tretë.
  5. Pohim numerik kundërshtues -> KiC, gjykim native (0,0), T1/T2.

Dy mbylljet përmbajnë rezultate të modelit, jo observime fizike të
Tokës. Dy paraqitje kanonike të modelit shtohen nga sistemi: gjurma
ka shtatë hyrje checkpoint, por vetëm pesë hyrje përdoruesi. Kjo nuk
është humbje ose shpikje e dy ndërhyrjeve njerëzore.

Rendi/gjykimi i burimit është ruajtur, përfshirë çiftin native 0/0 ose
1/1. Nuk është shpikur një format tjetër gjykimi për POC002.
Përkufizimi dhe formulat për kufijtë e provuar: AUTHORITY_CONTRACT.txt.

PREJARDHJA DHE PRETENDIMI

AUTORITETI AI si paradigmë arkitekturore propozohet nga Bledar Gjata
mbi metodën e ESS-MAI. Ky artefakt tregon një hierarki konkrete të
materializimit të saj në AIGovern-Cyber. Nuk ia atribuon si shpikje
të re makinat e gjendjeve, SHA-256, tipat konsumues, FFI ose testimin.
Kompilimi dhe testet mbështesin sjelljet e provuara; nuk vendosin
vetvetiu risinë akademike, sigurinë e përgjithshme apo mbylljen e
plotë të paradigmës. Kufijtë e native source mbeten të dukshëm.

RIPRODHIMI

Në këtë makinë: Rust/Cargo 1.98.0, Windows GNU, gcc MinGW, feature-t
default runtime_mode/hw_kernel. Nuk çaktivizohen warnings ose kontrolle.

Për kernel-in C, përdor gcc/AR/linker nga i njëjti MinGW dhe vendose
dosjen e tij bin përpara në PATH. Konfigurimi i provuar në PowerShell:
  $env:PATH = 'C:\msys64\mingw64\bin;' + $env:PATH
  $env:CC = 'C:\msys64\mingw64\bin\gcc.exe'
  $env:AR = 'C:\msys64\mingw64\bin\ar.exe'
  $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $env:CC

Nga kjo dosje:
  python verify_sources.py
  cd native/runtime
  cargo test --locked -p quantum-platform --lib ultimatum_collapse_law:: -- --test-threads=1
  cargo test --locked -p quantum-platform --lib r28_real_model_endpoints -- --test-threads=1
  cargo test --locked -p quantum-platform --bin quantum-platform r26_kie_history_input_tests:: -- --test-threads=1
  cargo test --locked -p quantum-platform --lib convergence_retains_canonical_order_uniqueness_and_replay_binding -- --test-threads=1
  cargo test --locked -p shadow_contracts --lib exact_missing_negative_is_eligible_but_complete_pair_is_not -- --test-threads=1
  cargo build --locked --release --workspace
  cargo build --locked --release -p quantum-platform --example pd_iz_input --example pd_recovery_inspect

Nga rrënja e POC-ut, duke treguar dosjen release të build-it:
  python run_continuity.py --build native/runtime/target/release --output experiment-new
  python verify_evidence.py --evidence experiment-new

Dosja --output duhet të mos ekzistojë. Driver-i krijon aty Vault-in,
handoff-in dhe logjet; nuk përdor memorien e produktit. Hardware-i
matet në momentin e ekzekutimit. Një HOLD i ri nuk anashkalohet.
Për kontrollin e evidencës së paketuar pa ekzekutuar sistemin:
  python verify_sources.py
  python verify_evidence.py

Në ekzekutimin e dokumentuar u përdor edhe --offline me cache ekzistues
dhe CARGO_TARGET_DIR jashtë paketës. Në një makinë të re Cargo mund
të shkarkojë varësitë e kyçura. Logjet dhe kufijtë e rezultateve janë
në RESULTS.txt; mos i zëvendëso me pritshmëritë nga emrat e testeve.

VERIFIKIMI I PLOTË PARA PUBLIKIMIT

1821 kaluar (përfshirë 4 doctests), 1 dështuar, 0 të injoruara; exit 101.
Dështon stdout-retention gjatë timeout-it 750 ms. Prova diagnostikuese
riprodhoi nisjen e ftohtë mbi 1 sekondë, pa hyrë fëmija në main.
Pragjet, testet dhe burimi native nuk u ndryshuan. README.md dhe
evidence/prepublication përmbajnë të dy dështimet dhe diagnozën.
Kjo është paketë kërkimore me kufij të deklaruar, jo produkt plotësisht green.
