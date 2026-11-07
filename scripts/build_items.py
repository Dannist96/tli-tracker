"""Builds src/data/items.json from the raw sources in data-src/.

Sources:
  - data-src/sattatracker_en_id_table.json  (MIT, see data-src/sattatracker_LICENSE.txt)
      English item names keyed by game base id.
  - data-src/etor_items_cn.json
      Item ids, Chinese type labels, icon URLs and tlidb.com links (English slug).

Output entries: {"<id>": {"n": English name, "c": category key, "i": icon file or ""}}
Category keys are translated in src/locales/*.json under "category.*".

Run with: python -I scripts/build_items.py
"""

import json
import re
import sys
import urllib.parse
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "data-src"
OUT = ROOT / "src" / "data" / "items.json"
# Chinese item name -> id, used to translate server-side strategy names.
CN_OUT = ROOT / "src" / "data" / "cn_names.json"

# Chinese type label (as used by the game data) -> our category key.
CATEGORY_BY_TYPE = {
    "传奇装备": "legendary",
    "主动技能": "skill",
    "辅助技能": "skill",
    "崇高辅助技能": "skill",
    "华贵辅助技能": "skill",
    "被动技能": "skill",
    "触媒技能": "skill",
    "模组化技能": "skill",
    "命运": "destiny",
    "未定宿命": "destiny",
    "命运相关": "destiny",
    "罗盘": "compass",
    "赛季罗盘": "compass",
    "星盘": "compass",
    "探针": "compass",
    "信标": "beacon",
    "记忆荧光": "memory",
    "复苏材料（月相）": "memory",
    "复苏材料": "memory",
    "追忆游丝": "memory",
    "追忆碎絮": "memory",
    "塔罗斯技能": "tartarus",
    "始源塔罗斯技能": "tartarus",
    "神格契约": "divinity",
    "神骸": "divinity",
    "神威纹章": "divinity",
    "神格残片": "divinity",
    "升华之楔": "divinity",
    "归一之楔": "divinity",
    "神威辉石": "divinity",
    "巨力之神": "divinity",
    "狩猎之神": "divinity",
    "知识之神": "divinity",
    "征战之神": "divinity",
    "欺诈之神": "divinity",
    "机械之神": "divinity",
    "冥王的神格": "divinity",
    "冥王的残缺神格": "divinity",
    "魂钉": "divinity",
    "异度棱镜": "prism",
    "特殊棱镜": "prism",
    "棱镜水平仪": "prism",
    "棱镜校尺": "prism",
    "棱镜修复仪": "prism",
    "逆像的残影": "prism",
}

ORGAN_PREFIXES = ("核心器官", "特殊器官", "完美", "普通器官")

CURRENCY_IDS = {"100300"}  # Flame Elementium, the price unit


def category_for(cn_type: str, item_id: str) -> str:
    if item_id in CURRENCY_IDS:
        return "currency"
    if cn_type in CATEGORY_BY_TYPE:
        return CATEGORY_BY_TYPE[cn_type]
    if cn_type.startswith(ORGAN_PREFIXES):
        return "organ"
    return "other"


def slug_name(url: str) -> str:
    if not url:
        return ""
    slug = urllib.parse.unquote(url.rstrip("/").rsplit("/", 1)[-1]).replace("_", " ").strip()
    # A slug with CJK characters is not an English name.
    return "" if re.search(r"[㐀-鿿]", slug) else slug


ICONS = ROOT / "public" / "icons"


def icon_file(item_id: str, img: str) -> str:
    """Icon file name; empty when not downloaded (see scripts/fetch_icons.py)."""
    if not img.startswith("https://cdn.tlidb.com/"):
        return ""
    ext = img.rsplit(".", 1)[-1].lower()
    name = f"{item_id}.{ext}" if ext in ("webp", "png") else ""
    return name if name and (not ICONS.exists() or (ICONS / name).exists()) else ""


def clean(name: str) -> str:
    return re.sub(r"\s+", " ", name.replace("\\n", " ")).strip()


def main() -> int:
    en = json.loads((SRC / "sattatracker_en_id_table.json").read_text(encoding="utf-8"))
    cn = json.loads((SRC / "etor_items_cn.json").read_text(encoding="utf-8"))

    # Ids reused across seasons appear more than once; the last record is the
    # current item, so its English slug wins over the older table name.
    occurrences = {}
    for rec in cn:
        occurrences[str(rec.get("id", ""))] = occurrences.get(str(rec.get("id", "")), 0) + 1

    out = {}
    cn_names = {}
    for rec in cn:
        item_id = str(rec.get("id", ""))
        if not item_id.isdigit():
            continue
        slug = slug_name(rec.get("url", ""))
        table = clean(en.get(item_id, {}).get("name", ""))
        name = slug if (occurrences[item_id] > 1 and slug) else (table or slug)
        if not name:
            continue
        cn_name = str(rec.get("name", "")).strip()
        if cn_name and len(cn_name) <= 40:
            cn_names[cn_name] = item_id
        out[item_id] = {
            "n": name,
            "c": category_for(rec.get("type", ""), item_id),
            "i": icon_file(item_id, rec.get("img", "")),
        }

    # Items that only the English table knows about (gear bases, currency, ...).
    for item_id, rec in en.items():
        if item_id in out or not item_id.isdigit():
            continue
        name = clean(rec.get("name", ""))
        if not name or re.search(r"[㐀-鿿]", name):
            continue
        out[item_id] = {"n": name, "c": "currency" if item_id in CURRENCY_IDS else "other", "i": ""}

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(dict(sorted(out.items(), key=lambda kv: int(kv[0]))), ensure_ascii=False), encoding="utf-8")
    CN_OUT.write_text(json.dumps(cn_names, ensure_ascii=False), encoding="utf-8")
    print(f"wrote {len(out)} items to {OUT.relative_to(ROOT)}, {len(cn_names)} names to {CN_OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
