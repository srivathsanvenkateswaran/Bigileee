#!/usr/bin/env python3
"""Generate src/data/bio.rs from a players_22.csv player dataset.

Only factual biography is imported: height, weight, preferred foot and age, matched to the
game's parody clubs by real club and shirt number. Ratings, body-type labels and images in the
dataset are deliberately ignored.

Usage: python3 tools/import_bio.py /path/to/players_22.csv
"""

import csv
import difflib
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Real clubs in the same order as each league file's clubs.
REAL = {
    "spud": ["Arsenal", "Aston Villa", "Brentford", "Brighton & Hove Albion", "Burnley", "Chelsea", "Crystal Palace",
             "Everton", "Leeds United", "Leicester City", "Liverpool", "Manchester City", "Manchester United",
             "Newcastle United", "Norwich City", "Southampton", "Tottenham Hotspur", "Watford", "West Ham United",
             "Wolverhampton Wanderers"],
    "burger": ["FC Bayern München", "Borussia Dortmund", "Bayer 04 Leverkusen", "RB Leipzig", "1. FC Union Berlin",
               "Sport-Club Freiburg", "1. FC Köln", "1. FSV Mainz 05", "TSG Hoffenheim", "Borussia Mönchengladbach",
               "Eintracht Frankfurt", "VfL Wolfsburg", "VfL Bochum 1848", "FC Augsburg", "VfB Stuttgart",
               "Hertha BSC", "DSC Arminia Bielefeld", "SpVgg Greuther Fürth"],
    "croissant": ["Paris Saint-Germain", "Olympique de Marseille", "AS Monaco", "Stade Rennais FC", "OGC Nice",
                  "RC Strasbourg Alsace", "Racing Club de Lens", "Olympique Lyonnais", "FC Nantes", "LOSC Lille",
                  "Stade Brestois 29", "Stade de Reims", "Montpellier HSC", "Angers SCO", "ESTAC Troyes",
                  "FC Lorient", "Clermont Foot 63", "AS Saint-Étienne", "FC Metz", "FC Girondins de Bordeaux"],
    "paella": ["Real Madrid CF", "FC Barcelona", "Atlético de Madrid", "Sevilla FC", "Real Betis Balompié",
               "Real Sociedad", "Villarreal CF", "Athletic Club de Bilbao", "Valencia CF", "CA Osasuna",
               "RC Celta de Vigo", "Rayo Vallecano", "Elche CF", "RCD Espanyol de Barcelona", "Getafe CF",
               "RCD Mallorca", "Cádiz CF", "Granada CF", "Levante Unión Deportiva", "Deportivo Alavés"],
    "pizza": ["AC Milan", "Inter", "Napoli", "Juventus", "Lazio", "Roma", "Fiorentina", "Atalanta", "Hellas Verona",
              "Torino", "Sassuolo", "Udinese", "Bologna", "Empoli", "Sampdoria", "Spezia", "Salernitana",
              "Cagliari", "Genoa", "Venezia"],
    "dosa": ["ATK Mohun Bagan FC", "SC East Bengal", "Bengaluru FC", "Chennaiyin FC", "FC Goa", "Hyderabad FC",
             "Jamshedpur FC", "Kerala Blasters FC", "Mumbai City FC", "NorthEast United FC", "Odisha FC"],
    "camel": ["Al Hilal", "Al Ittihad", "Al Nassr", "Al Shabab", "Al Taawoun", "Al Fateh", "Al Fayha", "Damac FC",
              "Al Raed", "Abha Club", "Al Ettifaq", "Al Batin", "Al Tai", "Al Ahli", "Al Faisaly", "Al Hazem"],
    "obesity": ["Atlanta United", "Austin FC", "Charlotte FC", "Chicago Fire", "FC Cincinnati", "Colorado Rapids",
                "Columbus Crew", "D.C. United", "FC Dallas", "Houston Dynamo", "Inter Miami CF", "LA Galaxy",
                "Los Angeles FC", "Minnesota United", "CF Montréal", "Nashville SC", "New England Revolution",
                "New York City FC", "New York Red Bulls", "Orlando City SC", "Philadelphia Union",
                "Portland Timbers", "Real Salt Lake", "San Jose Earthquakes", "Seattle Sounders FC",
                "Sporting Kansas City", "Toronto FC", "Vancouver Whitecaps FC"],
    "samba": ["Palmeiras", "Internacional", "Fluminense", "Corinthians", "Flamengo", "Athletico Paranaense",
              "Atlético Mineiro", "Fortaleza", "São Paulo", "América Mineiro", "Botafogo", "Santos", "Goiás",
              "Red Bull Bragantino", "Coritiba", "Cuiabá", "Ceará", "Atlético Goianiense", "Avaí", "Juventude"],
    "cheese": ["Ajax", "PSV", "Feyenoord", "FC Twente", "AZ Alkmaar", "Vitesse", "FC Utrecht", "SC Heerenveen",
               "SC Cambuur", "NEC Nijmegen", "FC Groningen", "Go Ahead Eagles", "Sparta Rotterdam",
               "Fortuna Sittard", "RKC Waalwijk", "Heracles Almelo", "Willem II", "PEC Zwolle"],
    "pastel": ["FC Porto", "Sporting CP", "SL Benfica", "SC Braga", "Gil Vicente FC", "Vitória de Guimarães",
               "CD Santa Clara", "FC Famalicão", "GD Estoril Praia", "Marítimo", "FC Paços de Ferreira",
               "Boavista FC", "Portimonense SC", "FC Vizela", "FC Arouca", "Moreirense FC", "CD Tondela",
               "Belenenses SAD"],
}


# The dataset's name for each league, so clubs only match within their own league.
DATASET_LEAGUE = {
    "spud": "English Premier League",
    "burger": "German 1. Bundesliga",
    "croissant": "French Ligue 1",
    "paella": "Spain Primera Division",
    "pizza": "Italian Serie A",
    "dosa": "Indian Super League",
    "camel": "Saudi Abdul L. Jameel League",
    "obesity": "USA Major League Soccer",
    "samba": "Campeonato Brasileiro Série A",
    "cheese": "Holland Eredivisie",
    "pastel": "Portuguese Liga ZON SAGRES",
}


def norm(s: str) -> str:
    s = unicodedata.normalize("NFKD", s).encode("ascii", "ignore").decode().lower()
    s = re.sub(r"\b(fc|cf|sc|ac|as|ca|cd|sl|rc|rcd|club|de|da|do|1\.|04|1848)\b", " ", s)
    return re.sub(r"[^a-z0-9]+", " ", s).strip()


def parse_clubs(path: Path):
    """(club name, [(pun, number)]) in file order."""
    text = path.read_text()
    clubs = []
    for block in re.split(r"\n\s*Club \{", text)[1:]:
        name = re.search(r'name: "([^"]+)"', block).group(1)
        players = [(m.group(1), int(m.group(2))) for m in re.finditer(r'p\("([^"]+)", \w+, (\d+),', block)]
        clubs.append((name, players))
    return clubs


def similar(a: str, b: str) -> float:
    return difflib.SequenceMatcher(None, norm(a), norm(b)).ratio()


def club_similar(candidate: str, real: str) -> float:
    """Name similarity, but a dataset club containing every word of the real name counts as a match."""
    words = set(norm(real).split())
    if words and words <= set(norm(candidate).split()):
        return 1.0
    return similar(candidate, real)


def main():
    rows = list(csv.DictReader(open(sys.argv[1], encoding="utf-8")))
    by_club = {}
    league_clubs = {}
    for r in rows:
        by_club.setdefault(r.get("club_name", ""), []).append(r)
        league_clubs.setdefault(r.get("league_name", ""), set()).add(r.get("club_name", ""))

    out, matched, total, unmatched_clubs, doubtful = [], 0, 0, [], []
    for league, reals in REAL.items():
        clubs = parse_clubs(ROOT / "src" / "data" / f"{league}.rs")
        assert len(clubs) == len(reals), f"{league}: {len(clubs)} clubs vs {len(reals)} real names"
        for (parody, players), real in zip(clubs, reals):
            pool = league_clubs.get(DATASET_LEAGUE[league], set())
            best = max(pool, key=lambda c: club_similar(c, real)) if pool else ""
            if club_similar(best, real) < 0.6:
                unmatched_clubs.append(f"{league}: {real} (closest: {best})")
                total += len(players)
                continue
            squad = by_club[best]
            for pun, num in players:
                total += 1
                cands = [r for r in squad if r.get("club_jersey_number", "").split(".")[0] == str(num)]
                # Shirt numbers can be stale; fall back to the most similar name in the squad.
                pick = max(cands, key=lambda r: similar(r["short_name"], pun)) if cands else None
                by_name = max(squad, key=lambda r: max(similar(r["short_name"], pun), similar(r.get("long_name", ""), pun)))
                name_score = lambda r: max(similar(r["short_name"], pun), similar(r.get("long_name", ""), pun))
                if pick is None or name_score(pick) < 0.35:
                    pick = by_name if name_score(by_name) >= 0.5 else None
                if pick is None:
                    continue
                # Puns keep enough of the real name that a correct match scores well; drop weak ones.
                if name_score(pick) < 0.45:
                    doubtful.append(f"{parody} #{num} {pun} ~ {pick['short_name']}")
                    continue
                h = int(float(pick["height_cm"]))
                w = int(float(pick["weight_kg"]))
                left = pick.get("preferred_foot", "Right").strip().lower() == "left"
                age = int(float(pick.get("age", 26)))
                out.append(f'    ("{parody}", {num}, Bio {{ height_cm: {h}, weight_kg: {w}, left_foot: {str(left).lower()}, age: {age} }}),')
                matched += 1

    src = [
        "//! Factual player biography (height, weight, strong foot, age), keyed by parody club and shirt",
        "//! number. Generated by tools/import_bio.py; do not edit by hand.",
        "",
        "#[derive(Clone, Copy, Debug)]",
        "pub struct Bio {",
        "    pub height_cm: u16,",
        "    pub weight_kg: u16,",
        "    pub left_foot: bool,",
        "    pub age: u8,",
        "}",
        "",
        "#[rustfmt::skip]",
        "pub static BIO: &[(&str, u8, Bio)] = &[",
        *out,
        "];",
        "",
        "pub fn lookup(club: &str, num: u8) -> Option<Bio> {",
        "    BIO.iter().find(|e| e.0 == club && e.1 == num).map(|e| e.2)",
        "}",
        "",
    ]
    (ROOT / "src" / "data" / "bio.rs").write_text("\n".join(src))
    print(f"matched {matched} of {total} players")
    if unmatched_clubs:
        print("clubs not found in the dataset:")
        for c in unmatched_clubs:
            print("  ", c)
    if doubtful:
        print(f"{len(doubtful)} weak name matches (dropped):")
        for d in doubtful[:40]:
            print("  ", d)


if __name__ == "__main__":
    main()
