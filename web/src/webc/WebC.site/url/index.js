import { newEl } from "../../../js/dom.js";
import headerNew from "../../../js/dom/header.js";
import { baseUrl } from "../../../js/route.js";
import "../../H1Txt.js";
import "../../Card.js";
import {
  DOC_LINKS,
  PERF_CONFIG,
  PERF_HAS_ROCKSDB,
  PERF_MATRIX_ROWS,
  PERF_META,
  PERF_NOTE,
  VERIFY_ROWS
} from "../../../conf/perf.js";

const GITHUB = "https://github.com/my-god-war/eloqstore2fkv",
  section = (title) => {
    const sec = newEl("section"),
      h2 = newEl("h2");
    sec.className = "HomeSection";
    h2.className = "O";
    h2.textContent = title;
    sec.append(h2);
    return sec;
  },
  cell = (tag, text, cls) => {
    const el = newEl(tag);
    el.textContent = text;
    if (cls) el.className = cls;
    return el;
  };

let res_li;

const page = () => {
  const el = newEl("main"),
    [header, h1, brief] = headerNew(),
    [badge, sub] = [newEl("span"), newEl("p")];

  el.className = "Home";

  header.className += " HomeHero";
  badge.className = "HomeBadge";
  badge.textContent = "C++ EloqStore ↔ Rust fkv";
  h1.textContent = "eloqstore2fkv";
  brief.textContent = "同机 NVMe 性能对比与兼容性验证";
  sub.className = "HomeSub";
  sub.textContent =
    "逐字节格式语料、双向引擎读写与 NVMe 崩溃恢复；下表为同一 runner、同一 workload 的 C++ / Rust 实测。";
  header.prepend(badge);
  header.append(sub);

  const perf_sec = section("性能矩阵"),
    meta = newEl("p"),
    table_wrap = newEl("div"),
    table = newEl("table"),
    thead = newEl("thead"),
    tbody = newEl("tbody"),
    tr_head = newEl("tr"),
    dash = (value) => (value == null || value === "" ? "—" : value);

  perf_sec.id = "perf";
  meta.className = "HomeMeta";
  meta.textContent = PERF_META;
  perf_sec.append(meta);

  table_wrap.className = "HomeTableWrap";
  table.className = "HomeTable";
  tr_head.append(cell("th", "阶段"), cell("th", "Rust fkv"), cell("th", "C++ EloqStore"));
  if (PERF_HAS_ROCKSDB) tr_head.append(cell("th", "RocksDB"));
  tr_head.append(cell("th", "fkv / C++"));
  if (PERF_HAS_ROCKSDB) tr_head.append(cell("th", "fkv / RocksDB"));
  thead.append(tr_head);
  for (const [phase, fkv, cpp, rocksdb, ratio, ratio_rocksdb, tone] of PERF_MATRIX_ROWS) {
    const tr = newEl("tr");
    tr.append(cell("td", phase), cell("td", dash(fkv)), cell("td", dash(cpp)));
    if (PERF_HAS_ROCKSDB) tr.append(cell("td", dash(rocksdb)));
    tr.append(cell("td", dash(ratio), tone === "bad" ? "HomeSlow" : "HomeSpeed"));
    if (PERF_HAS_ROCKSDB)
      tr.append(cell("td", dash(ratio_rocksdb), tone === "bad" ? "HomeSlow" : "HomeSpeed"));
    tbody.append(tr);
  }
  table.append(thead, tbody);
  table_wrap.append(table);

  const note = newEl("p");
  note.className = "HomeNote";
  const note_link = newEl("a");
  note_link.href = baseUrl("/doc/performance");
  note_link.textContent = "查看口径";
  note.append(PERF_NOTE + " ", note_link);
  // 配置分三块（机器 / 测试负载 / RocksDB 列），放表格下面，不再是单行灰字。
  const config_area = newEl("div");
  config_area.className = "HomeConfig";
  for (const [title, item_li] of Object.entries(PERF_CONFIG)) {
    const card = newEl("div");
    card.className = "HomeConfigCard";
    const h3 = newEl("h3");
    h3.textContent = title;
    const dl = newEl("dl");
    for (const [name, value] of item_li) {
      const dt = newEl("dt");
      dt.textContent = name;
      const dd = newEl("dd");
      dd.textContent = value;
      dl.append(dt, dd);
    }
    card.append(h3, dl);
    config_area.append(card);
  }

  perf_sec.append(table_wrap, config_area, note);

  const verify_sec = section("验证状态"),
    verify_bar = newEl("div");
  verify_bar.className = "HomeVerify";
  verify_bar.append(
    ...VERIFY_ROWS.map(([name, value]) => {
      const item = newEl("div"),
        label = newEl("span");
      item.className = "HomeVerifyItem";
      label.textContent = name;
      item.append(cell("b", value), label);
      return item;
    })
  );
  verify_sec.append(verify_bar);

  const doc_sec = section("文档"),
    docs = newEl("nav");
  docs.className = "HomeLinks";
  docs.append(
    ...DOC_LINKS.map(([key, title]) => {
      const a = newEl("a");
      a.href = baseUrl("/doc/" + key);
      a.textContent = title;
      return a;
    })
  );
  doc_sec.append(docs);

  const footer = newEl("footer"),
    github = newEl("a"),
    license = newEl("a");

  footer.className = "HomeFoot";
  github.href = GITHUB;
  github.textContent = "GitHub ↗";
  license.href = GITHUB + "/blob/main/LICENSE";
  license.textContent = "Apache-2.0";
  footer.append(github, license);

  el.append(header, perf_sec, verify_sec, doc_sec, footer);
  res_li = [Promise.resolve(el), () => {}];
  return res_li;
};

export default (url) => {
  if (!url) {
    return res_li || page();
  }
};
