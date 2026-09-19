const app = `
<aside class="side">
  <div class="brand"><span class="mark"></span><b>workbench</b></div>
  <div class="nav"><i></i>Overview</div>
  <div class="nav on"><i></i>Sessions<span class="n">3</span></div>
  <div class="nav"><i></i>Widgets</div>
  <div class="nav"><i></i>Golden tests<span class="n">312</span></div>
  <div class="nav"><i></i>Themes</div>
  <div class="nav"><i></i>Motion</div>
  <div class="nav"><i></i>Settings</div>
  <div class="spacer"></div>
  <div class="user"><span class="avatar"></span><div><b>Richard Huang</b><span>owner</span></div></div>
</aside>

<section class="main">
  <div class="head">
    <h1>Agent session</h1><span class="meta">session.jsonl, fixture</span>
    <div class="r"><span class="badge warn">paused</span><span class="btn">Reload</span><span class="btn primary focus">Resume</span></div>
  </div>

  <div class="stats">
    <div class="card stat"><div class="k">Tool calls</div><div class="v">15</div><div class="d"><span class="up">+4</span><span>since last turn</span></div></div>
    <div class="card stat"><div class="k">Output tokens</div><div class="v">13,516</div><div class="d"><span class="up">+12%</span><span>vs. previous session</span></div></div>
    <div class="card stat"><div class="k">Frame cost</div><div class="v">0.31 ms</div><div class="d"><span class="dn">−0.04</span><span>rebuild, layout, paint</span></div></div>
    <div class="card stat"><div class="k">Goldens</div><div class="v">312 / 312</div><div class="d"><span class="up">0 diffs</span><span>tolerance 3 of 255</span></div></div>
  </div>

  <div class="charts">
    <div class="card chart">
      <div class="t"><b>Output tokens per turn</b><span>last 14 turns</span><span class="seg"><span class="on">Turns</span><span>Time</span></span></div>
      <svg viewBox="0 0 900 300" width="100%" height="300">
        <g stroke="var(--grid)" stroke-width="1">
          <line x1="40" x2="890" y1="30" y2="30"/><line x1="40" x2="890" y1="95" y2="95"/><line x1="40" x2="890" y1="160" y2="160"/><line x1="40" x2="890" y1="225" y2="225"/><line x1="40" x2="890" y1="290" y2="290"/>
        </g>
        <g fill="var(--subtle)" font-size="15" text-anchor="end"><text x="30" y="35">2k</text><text x="30" y="100">1.5k</text><text x="30" y="165">1k</text><text x="30" y="230">500</text><text x="30" y="295">0</text></g>
        <defs><linearGradient id="fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="var(--accent)" stop-opacity=".28"/><stop offset="1" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs>
        <path d="M60,200 L125,150 L190,235 L255,60 L320,215 L385,120 L450,170 L515,240 L580,140 L645,190 L710,55 L775,160 L840,110 L890,135 L890,290 L60,290 Z" fill="url(#fill)"/>
        <path d="M60,200 L125,150 L190,235 L255,60 L320,215 L385,120 L450,170 L515,240 L580,140 L645,190 L710,55 L775,160 L840,110 L890,135" fill="none" stroke="var(--accent)" stroke-width="3.5" stroke-linejoin="round" stroke-linecap="round"/>
        <circle cx="710" cy="55" r="7" fill="var(--raised)" stroke="var(--accent)" stroke-width="3.5"/>
      </svg>
    </div>
    <div class="card chart">
      <div class="t"><b>Calls by tool</b><span>this session</span></div>
      <svg viewBox="0 0 640 300" width="100%" height="300">
        <g stroke="var(--grid)" stroke-width="1"><line x1="40" x2="630" y1="30" y2="30"/><line x1="40" x2="630" y1="95" y2="95"/><line x1="40" x2="630" y1="160" y2="160"/><line x1="40" x2="630" y1="225" y2="225"/><line x1="40" x2="630" y1="260" y2="260"/></g>
        <g fill="var(--subtle)" font-size="15" text-anchor="end"><text x="30" y="35">5</text><text x="30" y="100">4</text><text x="30" y="165">3</text><text x="30" y="230">2</text></g>
        <g fill="var(--accent)">
          <rect x="60" y="30" width="70" height="230" rx="6"/>
          <rect x="155" y="122" width="70" height="138" rx="6"/>
          <rect x="250" y="122" width="70" height="138" rx="6"/>
          <rect x="345" y="168" width="70" height="92" rx="6" opacity=".85"/>
          <rect x="440" y="214" width="70" height="46" rx="6" opacity=".7"/>
          <rect x="535" y="214" width="70" height="46" rx="6" opacity=".7"/>
        </g>
        <g fill="var(--muted)" font-size="16" text-anchor="middle"><text x="95" y="290">Bash</text><text x="190" y="290">Edit</text><text x="285" y="290">Read</text><text x="380" y="290">Write</text><text x="475" y="290">Grep</text><text x="570" y="290">Task</text></g>
      </svg>
    </div>
  </div>

  <div class="card table">
    <div class="t"><b>Calls</b><span>15, newest first</span></div>
    <div class="row h"><span>Time</span><span>Tool</span><span>What</span><span>Status</span><span class="num">Tokens</span></div>
    <div class="row"><span class="time">14:09:52</span><span><span class="badge acc">Grep</span></span><span class="path">MAX_TREE_DEPTH</span><span><i class="dot ok"></i>done</span><span class="num">212</span></div>
    <div class="row"><span class="time">14:09:15</span><span><span class="badge acc">Bash</span></span><span>Run A2UI conformance tests</span><span><i class="dot warn"></i>running</span><span class="num">1,940</span></div>
    <div class="row"><span class="time">14:08:40</span><span><span class="badge acc">Write</span></span><span class="path">fenestra-a2ui/src/render.rs</span><span><i class="dot ok"></i>done</span><span class="num">3,102</span></div>
    <div class="row"><span class="time">14:08:02</span><span><span class="badge acc">Read</span></span><span class="path">fenestra-describe/src/format.rs</span><span><i class="dot ok"></i>done</span><span class="num">88</span></div>
    <div class="row"><span class="time">14:07:30</span><span><span class="badge acc">Edit</span></span><span class="path">fenestra-describe/src/parse.rs</span><span><i class="dot ok"></i>done</span><span class="num">640</span></div>
    <div class="row"><span class="time">14:06:58</span><span><span class="badge acc">Task</span></span><span>Crash fix A: depth guard</span><span><i class="dot ok"></i>done</span><span class="num">54</span></div>
    <div class="row"><span class="time">14:06:20</span><span><span class="badge acc">Bash</span></span><span>Render the flagship examples headlessly</span><span><i class="dot ok"></i>done</span><span class="num">1,220</span></div>
  </div>
</section>

<aside class="rail">
  <div class="card">
    <b>Accent color</b>
    <div class="pad"><span class="knob"></span></div>
    <div class="strip hue"><span class="knob" style="left:72%"></span></div>
    <div class="strip alpha"><span class="knob" style="left:100%"></span></div>
    <div class="hexrow"><span class="swatch"></span><span class="input focus">#4477d9</span><span class="input">oklch 0.58 0.16 262</span></div>
  </div>
  <div class="card">
    <b>Controls</b>
    <div class="ctl">
      <div class="line"><span class="lab">Reduced motion</span><span class="switch"></span></div>
      <div class="line"><span class="lab">Embedded fonts</span><span class="switch"></span></div>
      <div class="line"><span class="check"></span><span class="lab">Compare against goldens</span><span class="kbd">⌘ G</span></div>
      <div class="line"><span class="radio on"></span><span class="lab">Metal</span><span class="radio"></span><span class="lab">lavapipe</span></div>
      <div class="line"><span class="lab" style="flex:none;width:110px">Scale 2.0</span><span class="slider"></span></div>
      <div class="line"><span class="lab" style="flex:none;width:110px">Rendering</span>
        <svg class="prog" viewBox="0 0 400 12" preserveAspectRatio="none" height="12"><path d="M0,6 q10,-6 20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0 t20,0" fill="none" stroke="var(--accent)" stroke-width="4" stroke-linecap="round"/><line x1="268" x2="400" y1="6" y2="6" stroke="var(--el-h)" stroke-width="4" stroke-linecap="round"/></svg>
      </div>
    </div>
  </div>
  <div class="card">
    <b>Golden tests</b>
    <div class="goldens">
      <div class="gold"><i class="dot ok"></i><span class="name">controls_light.png</span><span class="de">Δ 0</span></div>
      <div class="gold"><i class="dot ok"></i><span class="name">dashboard_dark.png</span><span class="de">Δ 1</span></div>
      <div class="gold"><i class="dot ok"></i><span class="name">agent_dashboard_light.png</span><span class="de">Δ 0</span></div>
      <div class="gold"><i class="dot ok"></i><span class="name">glass_modal_dark.png</span><span class="de">Δ 2</span></div>
    </div>
  </div>
  <div class="toast"><span class="ic"></span><div><b>Golden tests passed</b><span>312 images, none outside tolerance</span></div></div>
</aside>`;
for (const el of document.querySelectorAll('.app')) el.innerHTML = app;
