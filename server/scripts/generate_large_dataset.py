#!/usr/bin/env python3
"""
Generate a large dataset SQL file for Elidune legacy database testing.
"""

import random
import datetime
from typing import List, Tuple

# French first names and last names
FIRST_NAMES = [
    'Jean', 'Marie', 'Pierre', 'Sophie', 'Michel', 'Catherine', 'Philippe', 'Isabelle',
    'Alain', 'Françoise', 'Bernard', 'Monique', 'Daniel', 'Nicole', 'André', 'Jacqueline',
    'Claude', 'Françoise', 'Gérard', 'Martine', 'Henri', 'Sylvie', 'Louis', 'Brigitte',
    'Paul', 'Christine', 'Jacques', 'Pascale', 'Robert', 'Valérie', 'Marc', 'Sandrine',
    'Laurent', 'Céline', 'Nicolas', 'Julie', 'Stéphane', 'Caroline', 'Olivier', 'Aurélie',
    'David', 'Emilie', 'Thomas', 'Marion', 'Julien', 'Camille', 'Antoine', 'Laura',
    'Maxime', 'Clara', 'Alexandre', 'Léa', 'Romain', 'Emma', 'Vincent', 'Chloé',
    'Lucas', 'Manon', 'Hugo', 'Sarah', 'Léo', 'Pauline', 'Mathieu', 'Anaïs',
    'Baptiste', 'Lucie', 'Guillaume', 'Élise', 'Florian', 'Amélie', 'Adrien', 'Justine'
]

LAST_NAMES = [
    'Martin', 'Bernard', 'Dubois', 'Thomas', 'Robert', 'Richard', 'Petit', 'Durand',
    'Leroy', 'Moreau', 'Simon', 'Laurent', 'Lefebvre', 'Michel', 'Garcia', 'David',
    'Bertrand', 'Roux', 'Vincent', 'Fournier', 'Morel', 'Girard', 'André', 'Lefevre',
    'Mercier', 'Dupont', 'Lambert', 'Bonnet', 'François', 'Martinez', 'Legrand', 'Garnier',
    'Faure', 'Rousseau', 'Blanc', 'Guerin', 'Muller', 'Henry', 'Roussel', 'Nicolas',
    'Perrin', 'Morin', 'Mathieu', 'Clement', 'Gauthier', 'Dumont', 'Lopez', 'Fontaine',
    'Chevalier', 'Robin', 'Masson', 'Sanchez', 'Gerard', 'Nguyen', 'Boyer', 'Denis',
    'Lemaire', 'Dufour', 'Meyer', 'Perez', 'Gautier', 'Blanchard', 'Schmitt', 'Noel',
    'Brun', 'Giraud', 'Joly', 'Riviere', 'Lucas', 'Brunet', 'Colin', 'Arnaud'
]

CITIES = [
    ('Paris', 75001), ('Lyon', 69001), ('Marseille', 13001), ('Toulouse', 31000),
    ('Nice', 6000), ('Nantes', 44000), ('Strasbourg', 67000), ('Montpellier', 34000),
    ('Bordeaux', 33000), ('Lille', 59000), ('Rennes', 35000), ('Reims', 51100),
    ('Le Havre', 76600), ('Saint-Étienne', 42000), ('Toulon', 83000), ('Grenoble', 38000),
    ('Dijon', 21000), ('Angers', 49000), ('Nîmes', 30000), ('Villeurbanne', 69100)
]

OCCUPATIONS = [
    'Enseignant', 'Étudiant', 'Retraité', 'Ingénieur', 'Médecin', 'Avocat',
    'Comptable', 'Infirmier', 'Écolier', 'Artisan', 'Commerçant', 'Cadre',
    'Employé', 'Ouvrier', 'Fonctionnaire', 'Libéral', 'Chef d''entreprise'
]

# Book titles and authors for generating realistic data
BOOK_TITLES = [
    'L''Art de la guerre', 'Le Comte de Monte-Cristo', 'Notre-Dame de Paris',
    'Madame Bovary', 'L''Étranger', 'Le Rouge et le Noir', 'Germinal',
    'Bel-Ami', 'La Peste', 'Les Fleurs du mal', 'Candide', 'L''Assommoir',
    'Le Père Goriot', 'La Chartreuse de Parme', 'Les Liaisons dangereuses',
    'Le Grand Meaulnes', 'Voyage au bout de la nuit', 'La Condition humaine',
    'L''Écume des jours', 'Le Château', 'La Nausée', 'L''Homme révolté',
    'Les Mots', 'La Chute', 'Le Premier Homme', 'La Gloire de mon père',
    'Le Chien jaune', 'Maigret et le clochard', 'Les Trois Mousquetaires',
    'Vingt mille lieues sous les mers', 'Le Tour du monde en 80 jours',
    'De la Terre à la Lune', 'L''Île mystérieuse', 'Michel Strogoff',
    'Les Enfants du capitaine Grant', 'Robinson Crusoé', 'Gulliver',
    'Don Quichotte', 'L''Odyssée', 'L''Iliade', 'L''Énéide', 'Les Métamorphoses',
    'Roméo et Juliette', 'Hamlet', 'Macbeth', 'Othello', 'Le Roi Lear',
    'Faust', 'Les Misérables', 'Les Contemplations', 'Hernani', 'Ruy Blas',
    'Le Cid', 'Phèdre', 'Andromaque', 'Britannicus', 'Bérénice', 'Athalie',
    'Le Tartuffe', 'Le Misanthrope', 'Les Femmes savantes', 'L''École des femmes',
    'Dom Juan', 'Le Bourgeois gentilhomme', 'Les Précieuses ridicules',
    'Le Malade imaginaire', 'Les Fourberies de Scapin', 'L''Avare'
]

AUTHOR_LASTNAMES = [
    'Hugo', 'Dumas', 'Verne', 'Zola', 'Flaubert', 'Camus', 'Stendhal', 'Balzac',
    'Maupassant', 'Proust', 'Gide', 'Sartre', 'Camus', 'Malraux', 'Vian', 'Kafka',
    'Camus', 'Camus', 'Camus', 'Pagnol', 'Simenon', 'Dumas', 'Verne', 'Verne',
    'Verne', 'Verne', 'Verne', 'Defoe', 'Swift', 'Cervantes', 'Homère', 'Homère',
    'Virgile', 'Ovide', 'Shakespeare', 'Shakespeare', 'Shakespeare', 'Shakespeare',
    'Shakespeare', 'Goethe', 'Hugo', 'Hugo', 'Hugo', 'Hugo', 'Corneille', 'Racine',
    'Racine', 'Racine', 'Racine', 'Racine', 'Molière', 'Molière', 'Molière',
    'Molière', 'Molière', 'Molière', 'Molière', 'Molière', 'Molière', 'Molière'
]

AUTHOR_FIRSTNAMES = [
    'Victor', 'Alexandre', 'Jules', 'Émile', 'Gustave', 'Albert', 'Stendhal', 'Honoré de',
    'Guy de', 'Marcel', 'André', 'Jean-Paul', 'Albert', 'André', 'Boris', 'Franz',
    'Albert', 'Albert', 'Albert', 'Marcel', 'Georges', 'Alexandre', 'Jules', 'Jules',
    'Jules', 'Jules', 'Jules', 'Daniel', 'Jonathan', 'Miguel de', None, None,
    None, None, 'William', 'William', 'William', 'William', 'William', 'Johann Wolfgang',
    'Victor', 'Victor', 'Victor', 'Victor', 'Pierre', 'Jean', 'Jean', 'Jean',
    'Jean', 'Jean-Baptiste', 'Jean-Baptiste', 'Jean-Baptiste', 'Jean-Baptiste',
    'Jean-Baptiste', 'Jean-Baptiste', 'Jean-Baptiste', 'Jean-Baptiste', 'Jean-Baptiste'
]

SERIES_NAMES = [
    'Les Misérables', 'Le Seigneur des Anneaux', 'Harry Potter', 'Astérix',
    'Tintin', 'Les Mousquetaires', 'Les Aventures de Sherlock Holmes',
    'Les Chroniques de Narnia', 'Dune', 'Fondation', 'Le Trône de fer',
    'Les Annales du Disque-monde', 'La Roue du temps', 'Malazan',
    'Les Chroniques de la Lune noire', 'Largo Winch', 'Thorgal', 'XIII',
    'Blake et Mortimer', 'Lucky Luke', 'Les Schtroumpfs', 'Boule et Bill',
    'Gaston Lagaffe', 'Spirou et Fantasio', 'Marsupilami', 'Iznogoud',
    'Les Aventures de Tintin', 'Les 4 As', 'Les Profs', 'Les Tuniques bleues'
]

GENRES = [
    'Roman', 'Roman historique', 'Fantasy', 'Science-fiction', 'Policier',
    'Thriller', 'Romance', 'Biographie', 'Essai', 'Poésie', 'Théâtre',
    'Conte', 'Nouvelle', 'Bande dessinée', 'Manga', 'Documentaire'
]

SUBJECTS = [
    'Histoire', 'Philosophie', 'Littérature', 'Science', 'Art', 'Musique',
    'Voyage', 'Aventure', 'Guerre', 'Amour', 'Famille', 'Amitié', 'Nature',
    'Animaux', 'Mystère', 'Magie', 'Futur', 'Passé', 'Société', 'Politique'
]

# French public holidays 2024-2025
HOLIDAYS_2024 = [
    datetime.date(2024, 1, 1),   # New Year
    datetime.date(2024, 4, 1),   # Easter Monday
    datetime.date(2024, 5, 1),   # Labor Day
    datetime.date(2024, 5, 8),   # Victory Day
    datetime.date(2024, 5, 9),   # Ascension
    datetime.date(2024, 5, 20),  # Whit Monday
    datetime.date(2024, 7, 14),  # Bastille Day
    datetime.date(2024, 8, 15), # Assumption
    datetime.date(2024, 11, 1),  # All Saints
    datetime.date(2024, 11, 11), # Armistice
    datetime.date(2024, 12, 25), # Christmas
]

HOLIDAYS_2025 = [
    datetime.date(2025, 1, 1),   # New Year
    datetime.date(2025, 4, 21),  # Easter Monday
    datetime.date(2025, 5, 1),   # Labor Day
    datetime.date(2025, 5, 8),   # Victory Day
    datetime.date(2025, 5, 29),  # Ascension
    datetime.date(2025, 6, 9),   # Whit Monday
    datetime.date(2025, 7, 14),  # Bastille Day
    datetime.date(2025, 8, 15), # Assumption
    datetime.date(2025, 11, 1),  # All Saints
    datetime.date(2025, 11, 11), # Armistice
    datetime.date(2025, 12, 25), # Christmas
]

ALL_HOLIDAYS = HOLIDAYS_2024 + HOLIDAYS_2025


def is_workday(date: datetime.date) -> bool:
    """Check if a date is a workday (not Sunday and not a holiday)."""
    return date.weekday() != 6 and date not in ALL_HOLIDAYS


def unix_timestamp(date: datetime.date, hour: int = 9) -> int:
    """Convert a date to Unix timestamp."""
    dt = datetime.datetime.combine(date, datetime.time(hour, 0))
    return int(dt.timestamp())


def generate_users(num_users: int, start_id: int = 1) -> List[str]:
    """Generate user INSERT statements."""
    statements = []
    used_logins = set()
    
    for i in range(num_users):
        user_id = start_id + i
        firstname = random.choice(FIRST_NAMES)
        lastname = random.choice(LAST_NAMES)
        login = f"{firstname.lower()}.{lastname.lower()}{user_id}"
        
        # Ensure unique login
        while login in used_logins:
            login = f"{firstname.lower()}.{lastname.lower()}{user_id}_{random.randint(100, 999)}"
        used_logins.add(login)
        
        city, zip_code = random.choice(CITIES)
        sex_id = random.choice([1, 2])  # 1 = male, 2 = female
        account_type_id = random.choices([1, 2, 3], weights=[5, 85, 10])[0]  # Mostly readers
        public_type = random.choice([97, 106, 117])  # Adult, Youth, Senior
        occupation = random.choice(OCCUPATIONS) if account_type_id == 2 else None
        
        # Creation date in the past year
        crea_date = unix_timestamp(
            datetime.date.today() - datetime.timedelta(days=random.randint(1, 365)),
            random.randint(9, 17)
        )
        
        email = f"{login}@email.fr" if random.random() > 0.1 else None
        phone = f"06{random.randint(10000000, 99999999)}" if random.random() > 0.2 else None
        street = f"{random.randint(1, 200)} rue {random.choice(['de la', 'du', 'des', 'de'])} {random.choice(['République', 'Liberté', 'Paix', 'Église', 'Mairie', 'École'])}" if random.random() > 0.15 else None
        
        statements.append(
            f"({user_id}, '{login}', 'pass{user_id}', '{firstname}', '{lastname}', "
            f"{f"'{email}'" if email else "NULL"}, "
            f"{f"'{street}'" if street else "NULL"}, "
            f"{zip_code if street else "NULL"}, "
            f"{f"'{city}'" if street else "NULL"}, "
            f"{f"'{phone}'" if phone else "NULL"}, "
            f"{sex_id}, {account_type_id}, NULL, NULL, NULL, "
            f"'{firstname.upper()[:3]}{user_id:03d}', NULL, "
            f"{f"'{occupation}'" if occupation else "NULL"}, "
            f"{crea_date}, {crea_date}, NULL, NULL, 0, {public_type})"
        )
    
    return statements


def generate_authors(num_authors: int, start_id: int = 1) -> List[str]:
    """Generate author INSERT statements."""
    statements = []
    used_keys = set()
    
    for i in range(num_authors):
        author_id = start_id + i
        lastname = random.choice(AUTHOR_LASTNAMES)
        firstname = random.choice(AUTHOR_FIRSTNAMES) if random.random() > 0.1 else None
        
        # Generate unique key
        key = f"{lastname.lower()}_{firstname.lower() if firstname else 'unknown'}_{author_id}" if firstname else f"{lastname.lower()}_{author_id}"
        while key in used_keys:
            key = f"{key}_{random.randint(1, 999)}"
        used_keys.add(key)
        
        bio = f"Author {lastname}" if random.random() > 0.5 else None
        notes = f"Notes for {lastname}" if random.random() > 0.7 else None
        
        statements.append(
            f"({author_id}, '{key}', '{lastname}', "
            f"{f"'{firstname}'" if firstname else "NULL"}, "
            f"{f"'{bio}'" if bio else "NULL"}, "
            f"{f"'{notes}'" if notes else "NULL"})"
        )
    
    return statements


def generate_series(num_series: int, start_id: int = 1) -> List[str]:
    """Generate series INSERT statements."""
    statements = []
    used_keys = set()
    
    for i in range(num_series):
        series_id = start_id + i
        name = random.choice(SERIES_NAMES) if i < len(SERIES_NAMES) else f"Série {series_id}"
        key = name.lower().replace(' ', '_').replace("'", '').replace('-', '_')
        
        while key in used_keys:
            key = f"{key}_{random.randint(1, 999)}"
        used_keys.add(key)
        
        statements.append(f"({series_id}, '{key}', '{name}')")
    
    return statements


def generate_items(num_items: int, num_authors: int, num_series: int, num_editions: int, num_collections: int, start_id: int = 1) -> Tuple[List[str], List[int]]:
    """Generate item INSERT statements. Returns statements and list of item IDs with their specimen counts."""
    statements = []
    item_specimen_counts = {}  # item_id -> nb_specimens
    
    for i in range(num_items):
        item_id = start_id + i
        title = random.choice(BOOK_TITLES) if i < len(BOOK_TITLES) else f"Livre {item_id}"
        title2 = f"Tome {random.randint(1, 10)}" if random.random() > 0.7 else None
        
        # Random author(s)
        num_auth = random.choices([1, 2, 3], weights=[70, 25, 5])[0]
        author_ids = [random.randint(1, num_authors) for _ in range(num_auth)]
        author_functions = ['70'] * num_auth  # '70' = author
        
        # Random series (30% chance)
        serie_id = random.randint(1, num_series) if random.random() < 0.3 else None
        serie_vol = random.randint(1, 20) if serie_id else None
        
        # Random collection (40% chance)
        collection_id = random.randint(1, num_collections) if random.random() < 0.4 else None
        
        # Random publication date
        pub_year = random.randint(1800, 2024)
        edition_year = random.randint(pub_year, 2024)
        
        # Number of specimens per item (1-5)
        nb_specimens = random.randint(1, 5)
        item_specimen_counts[item_id] = nb_specimens
        
        # ISBN-like identification
        isbn = f"978-2-{random.randint(10, 99)}-{random.randint(100000, 999999)}-{random.randint(0, 9)}"
        
        genre = random.choice([1, 2, 3, 4, 5, 6])  # Various genres
        subject = random.choice(SUBJECTS)
        keywords = f"{title.lower()}, {subject.lower()}"
        
        crea_date = unix_timestamp(
            datetime.date.today() - datetime.timedelta(days=random.randint(1, 730)),
            random.randint(9, 17)
        )
        
        # Build author arrays
        auth1_ids = f"ARRAY[{author_ids[0]}]" if len(author_ids) > 0 else "NULL"
        auth1_func = f"'{author_functions[0]}'" if len(author_functions) > 0 else "NULL"
        auth2_ids = f"ARRAY[{author_ids[1]}]" if len(author_ids) > 1 else "NULL"
        auth2_func = f"'{author_functions[1]}'" if len(author_functions) > 1 else "NULL"
        auth3_ids = f"ARRAY[{author_ids[2]}]" if len(author_ids) > 2 else "NULL"
        auth3_func = f"'{author_functions[2]}'" if len(author_functions) > 2 else "NULL"
        
        statements.append(
            f"({item_id}, 'b', '{isbn}', NULL, NULL, NULL, "
            f"'{pub_year}', 1, 1, "
            f"{f"'{title}'" if title else "NULL"}, "
            f"{f"'{title2}'" if title2 else "NULL"}, NULL, NULL, "
            f"{auth1_ids}, {auth1_func}, {auth2_ids}, {auth2_func}, {auth3_ids}, {auth3_func}, "
            f"{serie_id if serie_id else 'NULL'}, {serie_vol if serie_vol else 'NULL'}, "
            f"{collection_id if collection_id else 'NULL'}, NULL, NULL, "
            f"{random.randint(1, 4)}, NULL, NULL, {genre}, '{subject}', "
            f"{random.choice([97, 106])}, {random.randint(1, 8)}, '{edition_year}', "
            f"'{random.randint(100, 800)} p.', NULL, NULL, NULL, NULL, NULL, '{keywords}', "
            f"{nb_specimens}, NULL, 0, NULL, 1, {crea_date}, {crea_date})"
        )
    
    return statements, item_specimen_counts


def generate_specimens(item_specimen_counts: dict, start_specimen_id: int = 1) -> Tuple[List[str], int, dict]:
    """Generate specimen INSERT statements.
    
    Returns:
        Tuple of (statements, next_specimen_id, specimen_item_map)
        specimen_item_map only includes borrowable specimens (status 98)
    """
    statements = []
    specimen_id = start_specimen_id
    specimen_item_map = {}  # specimen_id -> item_id (only borrowable ones)
    
    for item_id, nb_specimens in item_specimen_counts.items():
        for j in range(nb_specimens):
            source_id = random.randint(1, 4)
            identification = f"LIV-{item_id:05d}-{chr(65+j)}"  # A, B, C, etc.
            cote = f"R ITM {item_id}"
            place = random.randint(1, 5)
            status = 98 if random.random() > 0.05 else 110  # 5% not borrowable
            price = f"{random.randint(5, 25)}.{random.randint(0, 99):02d}" if random.random() > 0.3 else None
            
            crea_date = unix_timestamp(
                datetime.date.today() - datetime.timedelta(days=random.randint(1, 365)),
                random.randint(9, 17)
            )
            
            statements.append(
                f"({specimen_id}, {item_id}, {source_id}, '{identification}', "
                f"'{cote}', {place}, {status}, "
                f"{f"'{price}'" if price else "NULL"}, {crea_date}, {crea_date})"
            )
            # Only add borrowable specimens to the map
            if status == 98:
                specimen_item_map[specimen_id] = item_id
            specimen_id += 1
    
    return statements, specimen_id, specimen_item_map


def generate_loans(num_users: int, specimen_item_map: dict, start_date: datetime.date, end_date: datetime.date) -> Tuple[List[str], List[str]]:
    """Generate loan and return INSERT statements for workdays only.
    
    Args:
        num_users: Number of users
        specimen_item_map: Dictionary mapping specimen_id -> item_id
        start_date: Start date for loans
        end_date: End date for loans
    """
    current_loans = []
    returned_loans = []
    loan_id = 1
    specimen_available = list(specimen_item_map.keys())
    specimen_loaned = {}  # specimen_id -> (user_id, loan_date, item_id)
    
    current_date = start_date
    while current_date <= end_date:
        if not is_workday(current_date):
            current_date += datetime.timedelta(days=1)
            continue
        
        # Number of loans for this day (5-40)
        num_loans = random.randint(5, 40)
        num_returns = random.randint(5, 40)
        
        # Generate returns first (free up specimens)
        for _ in range(min(num_returns, len(specimen_loaned))):
            if not specimen_loaned:
                break
            
            specimen_id = random.choice(list(specimen_loaned.keys()))
            user_id, loan_date, item_id = specimen_loaned.pop(specimen_id)
            specimen_available.append(specimen_id)
            
            # Return date (same day or later, but not before loan date)
            return_date = current_date
            if random.random() > 0.3:  # 70% returned on time or early
                # Return can be up to 2 days before current date, but not before loan date
                days_before = random.randint(0, 2)
                return_date = current_date - datetime.timedelta(days=days_before)
                if return_date < loan_date:
                    return_date = loan_date + datetime.timedelta(days=random.randint(1, 30))
            
            loan_timestamp = unix_timestamp(loan_date, random.randint(9, 17))
            issue_timestamp = unix_timestamp(loan_date + datetime.timedelta(days=random.randint(0, 3)), random.randint(9, 17))
            return_timestamp = unix_timestamp(return_date, random.randint(14, 18))
            
            nb_renews = random.randint(0, 2) if random.random() > 0.7 else 0
            
            returned_loans.append(
                f"({loan_id}, {user_id}, {specimen_id}, {item_id}, {loan_timestamp}, "
                f"{issue_timestamp}, {nb_renews}, {return_timestamp}, NULL)"
            )
            loan_id += 1
        
        # Generate new loans
        for _ in range(min(num_loans, len(specimen_available))):
            if not specimen_available:
                break
            
            specimen_id = random.choice(specimen_available)
            specimen_available.remove(specimen_id)
            
            user_id = random.randint(1, num_users)
            item_id = specimen_item_map[specimen_id]
            
            loan_timestamp = unix_timestamp(current_date, random.randint(9, 17))
            issue_timestamp = unix_timestamp(current_date + datetime.timedelta(days=random.randint(0, 3)), random.randint(9, 17))
            
            # 80% chance this will be returned later, 20% still active
            if random.random() < 0.8:
                specimen_loaned[specimen_id] = (user_id, current_date, item_id)
            else:
                # Active loan
                nb_renews = random.randint(0, 1) if random.random() > 0.8 else 0
                current_loans.append(
                    f"({loan_id}, {user_id}, {specimen_id}, {item_id}, {loan_timestamp}, "
                    f"{issue_timestamp}, {nb_renews}, NULL)"
                )
                loan_id += 1
        
        current_date += datetime.timedelta(days=1)
    
    # Add remaining active loans
    for specimen_id, (user_id, loan_date, item_id) in specimen_loaned.items():
        loan_timestamp = unix_timestamp(loan_date, random.randint(9, 17))
        issue_timestamp = unix_timestamp(loan_date + datetime.timedelta(days=random.randint(0, 3)), random.randint(9, 17))
        nb_renews = random.randint(0, 1) if random.random() > 0.8 else 0
        current_loans.append(
            f"({loan_id}, {user_id}, {specimen_id}, {item_id}, {loan_timestamp}, "
            f"{issue_timestamp}, {nb_renews}, NULL)"
        )
        loan_id += 1
    
    return current_loans, returned_loans


def main():
    num_users = 200
    num_items = 10000
    num_authors = 500
    num_series = 50
    num_editions = 20
    num_collections = 30
    
    # Date range for loans (last 6 months)
    end_date = datetime.date.today()
    start_date = end_date - datetime.timedelta(days=180)
    
    print("Generating SQL file...")
    
    sql_lines = []
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- Elidune Legacy Database Large Dataset")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- Generated dataset with:")
    sql_lines.append(f"--   - {num_users} users")
    sql_lines.append(f"--   - {num_items} books")
    sql_lines.append(f"--   - {num_authors} authors")
    sql_lines.append(f"--   - {num_series} series")
    sql_lines.append(f"--   - Daily loans (5-40 per workday)")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    
    # Include table creation from original file
    sql_lines.append("-- Cleanup if tables exist")
    sql_lines.append("DROP TABLE IF EXISTS borrows_archives CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS borrows CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS borrows_settings CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS remote_specimens CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS specimens CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS remote_items CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS items CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS z3950servers CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS fees CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS users CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS account_types CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS authors CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS editions CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS collections CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS series CASCADE;")
    sql_lines.append("DROP TABLE IF EXISTS sources CASCADE;")
    sql_lines.append("")
    
    # Reference tables (same as original)
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- REFERENCE TABLES")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE account_types (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    name VARCHAR,")
    sql_lines.append("    items_rights CHAR(1) DEFAULT 'n',")
    sql_lines.append("    users_rights CHAR(1) DEFAULT 'n',")
    sql_lines.append("    loans_rights CHAR(1) DEFAULT 'n',")
    sql_lines.append("    items_archive_rights CHAR(1) DEFAULT 'n',")
    sql_lines.append("    borrows_rights CHAR(1),")
    sql_lines.append("    settings_rights CHAR(1)")
    sql_lines.append(");")
    sql_lines.append("")
    sql_lines.append("INSERT INTO account_types (id, name, items_rights, users_rights, loans_rights, items_archive_rights, borrows_rights, settings_rights) VALUES")
    sql_lines.append("(1, 'Guest', 'r', 'r', 'n', 'n', 'n', 'r'),")
    sql_lines.append("(2, 'Reader', 'r', 'r', 'r', 'r', 'r', 'r'),")
    sql_lines.append("(3, 'Librarian', 'w', 'w', 'w', 'w', 'w', 'r'),")
    sql_lines.append("(4, 'Administrator', 'w', 'w', 'w', 'w', 'w', 'w'),")
    sql_lines.append("(8, 'Group', 'r', 'r', 'r', 'r', 'r', 'r');")
    sql_lines.append("")
    sql_lines.append("SELECT setval('account_types_id_seq', 10);")
    sql_lines.append("")
    
    # Users
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- USERS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE users (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    login VARCHAR,")
    sql_lines.append("    password VARCHAR,")
    sql_lines.append("    firstname VARCHAR,")
    sql_lines.append("    lastname VARCHAR,")
    sql_lines.append("    email VARCHAR,")
    sql_lines.append("    addr_street VARCHAR,")
    sql_lines.append("    addr_zip_code INTEGER,")
    sql_lines.append("    addr_city VARCHAR,")
    sql_lines.append("    phone VARCHAR,")
    sql_lines.append("    sex_id SMALLINT,")
    sql_lines.append("    account_type_id SMALLINT,")
    sql_lines.append("    subscription_type_id SMALLINT,")
    sql_lines.append("    fee_id SMALLINT,")
    sql_lines.append("    last_payement_date TIMESTAMP DEFAULT NOW(),")
    sql_lines.append("    group_id INTEGER,")
    sql_lines.append("    barcode VARCHAR,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    occupation VARCHAR,")
    sql_lines.append("    crea_date INTEGER,")
    sql_lines.append("    modif_date INTEGER,")
    sql_lines.append("    issue_date INTEGER,")
    sql_lines.append("    birthdate VARCHAR,")
    sql_lines.append("    archived_date INTEGER DEFAULT 0,")
    sql_lines.append("    public_type INTEGER")
    sql_lines.append(");")
    sql_lines.append("")
    
    user_statements = generate_users(num_users)
    sql_lines.append("INSERT INTO users (id, login, password, firstname, lastname, email, addr_street, addr_zip_code, addr_city, phone, sex_id, account_type_id, subscription_type_id, fee_id, group_id, barcode, notes, occupation, crea_date, modif_date, issue_date, birthdate, archived_date, public_type) VALUES")
    for i, stmt in enumerate(user_statements):
        sql_lines.append(f"{stmt}{',' if i < len(user_statements) - 1 else ';'}")
    sql_lines.append("")
    sql_lines.append(f"SELECT setval('users_id_seq', {num_users + 10});")
    sql_lines.append("")
    
    # Authors
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- AUTHORS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE authors (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    key VARCHAR UNIQUE,")
    sql_lines.append("    lastname VARCHAR,")
    sql_lines.append("    firstname VARCHAR,")
    sql_lines.append("    bio VARCHAR,")
    sql_lines.append("    notes VARCHAR")
    sql_lines.append(");")
    sql_lines.append("")
    
    author_statements = generate_authors(num_authors)
    sql_lines.append("INSERT INTO authors (id, key, lastname, firstname, bio, notes) VALUES")
    for i, stmt in enumerate(author_statements):
        sql_lines.append(f"{stmt}{',' if i < len(author_statements) - 1 else ';'}")
    sql_lines.append("")
    sql_lines.append(f"SELECT setval('authors_id_seq', {num_authors + 10});")
    sql_lines.append("")
    
    # Editions (publishers)
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- PUBLISHERS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE editions (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    key VARCHAR,")
    sql_lines.append("    name VARCHAR,")
    sql_lines.append("    place VARCHAR,")
    sql_lines.append("    notes VARCHAR")
    sql_lines.append(");")
    sql_lines.append("")
    
    edition_names = ['Gallimard', 'Folio', 'Pocket', 'Le Livre de Poche', 'Dargaud', 'Casterman', 'Hachette', 'Flammarion', 'Seuil', 'Albin Michel', 'Grasset', 'Fayard', 'LGF', 'J''ai lu', '10/18', 'Points', 'Actes Sud', 'Robert Laffont', 'Stock', 'Denoël']
    sql_lines.append("INSERT INTO editions (id, key, name, place, notes) VALUES")
    for i in range(num_editions):
        name = edition_names[i] if i < len(edition_names) else f"Édition {i+1}"
        key = name.lower().replace(' ', '_').replace("'", '').replace('-', '_')
        sql_lines.append(f"({i+1}, '{key}', '{name}', 'Paris', NULL){',' if i < num_editions - 1 else ';'}")
    sql_lines.append("")
    sql_lines.append(f"SELECT setval('editions_id_seq', {num_editions + 10});")
    sql_lines.append("")
    
    # Collections
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- COLLECTIONS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE collections (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    key VARCHAR,")
    sql_lines.append("    title1 VARCHAR,")
    sql_lines.append("    title2 VARCHAR,")
    sql_lines.append("    title3 VARCHAR,")
    sql_lines.append("    issn VARCHAR")
    sql_lines.append(");")
    sql_lines.append("")
    
    collection_names = ['Folio Classique', 'Bibliothèque de la Pléiade', 'Harry Potter', 'Astérix', 'Les Aventures de Tintin', 'Policier', 'Science-fiction', 'Fantasy', 'Roman', 'Essai', 'Poésie', 'Théâtre', 'Biographie', 'Histoire', 'Philosophie', 'Art', 'Musique', 'Voyage', 'Nature', 'Jeunesse', 'BD', 'Manga', 'Poche', 'Grand format', 'Luxe', 'Illustré', 'Coffret', 'Spécial', 'Collector', 'Édition limitée']
    sql_lines.append("INSERT INTO collections (id, key, title1, title2, title3, issn) VALUES")
    for i in range(num_collections):
        name = collection_names[i] if i < len(collection_names) else f"Collection {i+1}"
        key = name.lower().replace(' ', '_').replace("'", '').replace('-', '_')
        sql_lines.append(f"({i+1}, '{key}', '{name}', NULL, NULL, NULL){',' if i < num_collections - 1 else ';'}")
    sql_lines.append("")
    sql_lines.append(f"SELECT setval('collections_id_seq', {num_collections + 10});")
    sql_lines.append("")
    
    # Series
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- SERIES")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE series (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    key VARCHAR,")
    sql_lines.append("    name VARCHAR")
    sql_lines.append(");")
    sql_lines.append("")
    
    series_statements = generate_series(num_series)
    sql_lines.append("INSERT INTO series (id, key, name) VALUES")
    for i, stmt in enumerate(series_statements):
        sql_lines.append(f"{stmt}{',' if i < len(series_statements) - 1 else ';'}")
    sql_lines.append("")
    sql_lines.append(f"SELECT setval('series_id_seq', {num_series + 10});")
    sql_lines.append("")
    
    # Sources
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- SOURCES")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE sources (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    key VARCHAR,")
    sql_lines.append("    name VARCHAR")
    sql_lines.append(");")
    sql_lines.append("")
    sql_lines.append("INSERT INTO sources (id, key, name) VALUES")
    sql_lines.append("(1, 'achat', 'Purchase'),")
    sql_lines.append("(2, 'don', 'Donation'),")
    sql_lines.append("(3, 'depot', 'Legal deposit'),")
    sql_lines.append("(4, 'echange', 'Exchange');")
    sql_lines.append("")
    sql_lines.append("SELECT setval('sources_id_seq', 10);")
    sql_lines.append("")
    
    # Items
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- ITEMS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE items (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    media_type VARCHAR,")
    sql_lines.append("    identification VARCHAR,")
    sql_lines.append("    price VARCHAR,")
    sql_lines.append("    barcode VARCHAR,")
    sql_lines.append("    dewey VARCHAR,")
    sql_lines.append("    publication_date VARCHAR,")
    sql_lines.append("    lang SMALLINT,")
    sql_lines.append("    lang_orig SMALLINT,")
    sql_lines.append("    title1 VARCHAR,")
    sql_lines.append("    title2 VARCHAR,")
    sql_lines.append("    title3 VARCHAR,")
    sql_lines.append("    title4 VARCHAR,")
    sql_lines.append("    author1_ids INTEGER[],")
    sql_lines.append("    author1_functions VARCHAR,")
    sql_lines.append("    author2_ids INTEGER[],")
    sql_lines.append("    author2_functions VARCHAR,")
    sql_lines.append("    author3_ids INTEGER[],")
    sql_lines.append("    author3_functions VARCHAR,")
    sql_lines.append("    serie_id INTEGER,")
    sql_lines.append("    serie_vol_number SMALLINT,")
    sql_lines.append("    collection_id INTEGER,")
    sql_lines.append("    collection_number_sub SMALLINT,")
    sql_lines.append("    collection_vol_number SMALLINT,")
    sql_lines.append("    source_id INTEGER,")
    sql_lines.append("    source_date VARCHAR,")
    sql_lines.append("    source_norme VARCHAR,")
    sql_lines.append("    genre SMALLINT,")
    sql_lines.append("    subject VARCHAR,")
    sql_lines.append("    public_type SMALLINT,")
    sql_lines.append("    edition_id INTEGER,")
    sql_lines.append("    edition_date VARCHAR,")
    sql_lines.append("    nb_pages VARCHAR,")
    sql_lines.append("    format VARCHAR,")
    sql_lines.append("    content VARCHAR,")
    sql_lines.append("    addon VARCHAR,")
    sql_lines.append("    abstract VARCHAR,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    keywords VARCHAR,")
    sql_lines.append("    nb_specimens SMALLINT DEFAULT 0,")
    sql_lines.append("    state VARCHAR,")
    sql_lines.append("    is_archive SMALLINT DEFAULT 0,")
    sql_lines.append("    archived_timestamp INTEGER,")
    sql_lines.append("    is_valid SMALLINT DEFAULT 1,")
    sql_lines.append("    crea_date INTEGER,")
    sql_lines.append("    modif_date INTEGER")
    sql_lines.append(");")
    sql_lines.append("")
    
    print("Generating items...")
    item_statements, item_specimen_counts = generate_items(num_items, num_authors, num_series, num_editions, num_collections)
    
    # Write items in batches
    batch_size = 500
    for batch_start in range(0, len(item_statements), batch_size):
        batch_end = min(batch_start + batch_size, len(item_statements))
        batch = item_statements[batch_start:batch_end]
        sql_lines.append("INSERT INTO items (id, media_type, identification, price, barcode, dewey, publication_date, lang, lang_orig, title1, title2, title3, title4, author1_ids, author1_functions, author2_ids, author2_functions, author3_ids, author3_functions, serie_id, serie_vol_number, collection_id, collection_number_sub, collection_vol_number, source_id, source_date, source_norme, genre, subject, public_type, edition_id, edition_date, nb_pages, format, content, addon, abstract, notes, keywords, nb_specimens, state, is_archive, archived_timestamp, is_valid, crea_date, modif_date) VALUES")
        for i, stmt in enumerate(batch):
            sql_lines.append(f"{stmt}{',' if i < len(batch) - 1 else ';'}")
        sql_lines.append("")
    
    sql_lines.append(f"SELECT setval('items_id_seq', {num_items + 10});")
    sql_lines.append("")
    
    # Specimens
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- SPECIMENS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE specimens (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    id_item INTEGER,")
    sql_lines.append("    source_id INTEGER,")
    sql_lines.append("    identification VARCHAR,")
    sql_lines.append("    cote VARCHAR,")
    sql_lines.append("    place SMALLINT,")
    sql_lines.append("    status SMALLINT DEFAULT 98,")
    sql_lines.append("    codestat SMALLINT,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    price VARCHAR,")
    sql_lines.append("    modif_date INTEGER,")
    sql_lines.append("    is_archive INTEGER DEFAULT 0,")
    sql_lines.append("    archive_date INTEGER DEFAULT 0,")
    sql_lines.append("    crea_date INTEGER")
    sql_lines.append(");")
    sql_lines.append("")
    
    print("Generating specimens...")
    specimen_statements, next_specimen_id, specimen_item_map = generate_specimens(item_specimen_counts)
    num_specimens = next_specimen_id - 1
    
    # Write specimens in batches
    batch_size = 500
    for batch_start in range(0, len(specimen_statements), batch_size):
        batch_end = min(batch_start + batch_size, len(specimen_statements))
        batch = specimen_statements[batch_start:batch_end]
        sql_lines.append("INSERT INTO specimens (id, id_item, source_id, identification, cote, place, status, price, crea_date, modif_date) VALUES")
        for i, stmt in enumerate(batch):
            sql_lines.append(f"{stmt}{',' if i < len(batch) - 1 else ';'}")
        sql_lines.append("")
    
    sql_lines.append(f"SELECT setval('specimens_id_seq', {num_specimens + 10});")
    sql_lines.append("")
    
    # Loans
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- LOANS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE borrows (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    user_id INTEGER NOT NULL,")
    sql_lines.append("    specimen_id INTEGER NOT NULL,")
    sql_lines.append("    item_id INTEGER,")
    sql_lines.append("    date INTEGER NOT NULL,")
    sql_lines.append("    renew_date INTEGER,")
    sql_lines.append("    nb_renews SMALLINT DEFAULT 0,")
    sql_lines.append("    issue_date INTEGER,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    returned_date INTEGER")
    sql_lines.append(");")
    sql_lines.append("")
    
    print("Generating loans (this may take a while)...")
    current_loans, returned_loans = generate_loans(num_users, specimen_item_map, start_date, end_date)
    
    # Write returned loans first
    if returned_loans:
        batch_size = 500
        for batch_start in range(0, len(returned_loans), batch_size):
            batch_end = min(batch_start + batch_size, len(returned_loans))
            batch = returned_loans[batch_start:batch_end]
            sql_lines.append("INSERT INTO borrows (id, user_id, specimen_id, item_id, date, issue_date, nb_renews, returned_date, notes) VALUES")
            for i, stmt in enumerate(batch):
                sql_lines.append(f"{stmt}{',' if i < len(batch) - 1 else ';'}")
            sql_lines.append("")
    
    # Write current loans
    if current_loans:
        batch_size = 500
        for batch_start in range(0, len(current_loans), batch_size):
            batch_end = min(batch_start + batch_size, len(current_loans))
            batch = current_loans[batch_start:batch_end]
            sql_lines.append("INSERT INTO borrows (id, user_id, specimen_id, item_id, date, issue_date, nb_renews, notes) VALUES")
            for i, stmt in enumerate(batch):
                sql_lines.append(f"{stmt}{',' if i < len(batch) - 1 else ';'}")
            sql_lines.append("")
    
    total_loans = len(current_loans) + len(returned_loans)
    sql_lines.append(f"SELECT setval('borrows_id_seq', {total_loans + 10});")
    sql_lines.append("")
    
    # Other tables (same as original)
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- LOAN ARCHIVES")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE borrows_archives (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    item_id INTEGER NOT NULL,")
    sql_lines.append("    specimen_id INTEGER,")
    sql_lines.append("    date INTEGER NOT NULL,")
    sql_lines.append("    nb_renews SMALLINT,")
    sql_lines.append("    issue_date INTEGER,")
    sql_lines.append("    returned_date INTEGER,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    borrower_public_type INTEGER,")
    sql_lines.append("    occupation VARCHAR,")
    sql_lines.append("    addr_city VARCHAR,")
    sql_lines.append("    sex_id SMALLINT,")
    sql_lines.append("    account_type_id SMALLINT")
    sql_lines.append(");")
    sql_lines.append("")
    sql_lines.append("SELECT setval('borrows_archives_id_seq', 10);")
    sql_lines.append("")
    
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- LOAN SETTINGS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE borrows_settings (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    media_type VARCHAR,")
    sql_lines.append("    nb_max SMALLINT,")
    sql_lines.append("    nb_renews SMALLINT,")
    sql_lines.append("    duration SMALLINT,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    account_type_id SMALLINT")
    sql_lines.append(");")
    sql_lines.append("")
    sql_lines.append("INSERT INTO borrows_settings (id, media_type, nb_max, nb_renews, duration, notes) VALUES")
    sql_lines.append("(1, 'b', 5, 2, 21, 'Books'),")
    sql_lines.append("(2, 'bc', 5, 1, 14, 'Comics'),")
    sql_lines.append("(3, 'p', 3, 0, 7, 'Periodicals'),")
    sql_lines.append("(4, 'amc', 3, 1, 14, 'Audio CDs'),")
    sql_lines.append("(5, 'vd', 2, 1, 7, 'DVDs');")
    sql_lines.append("")
    sql_lines.append("SELECT setval('borrows_settings_id_seq', 10);")
    sql_lines.append("")
    
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- Z39.50 SERVERS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE z3950servers (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    address VARCHAR,")
    sql_lines.append("    port INTEGER DEFAULT 2200,")
    sql_lines.append("    name VARCHAR,")
    sql_lines.append("    description VARCHAR,")
    sql_lines.append("    activated INTEGER DEFAULT 0,")
    sql_lines.append("    login VARCHAR,")
    sql_lines.append("    password VARCHAR,")
    sql_lines.append("    database VARCHAR,")
    sql_lines.append("    format VARCHAR,")
    sql_lines.append("    encoding VARCHAR DEFAULT 'utf-8'")
    sql_lines.append(");")
    sql_lines.append("")
    sql_lines.append("INSERT INTO z3950servers (id, name, address, port, database, format, activated, description) VALUES")
    sql_lines.append("(1, 'BnF - General Catalog', 'z3950.bnf.fr', 2211, 'TOUT-UTF8', 'UNIMARC', 1, 'French National Library'),")
    sql_lines.append("(2, 'SUDOC', 'z3950.sudoc.fr', 2100, 'default', 'UNIMARC', 1, 'French University Documentation System'),")
    sql_lines.append("(3, 'Library of Congress', 'z3950.loc.gov', 7090, 'VOYAGER', 'MARC21', 0, 'US Library of Congress');")
    sql_lines.append("")
    sql_lines.append("SELECT setval('z3950servers_id_seq', 10);")
    sql_lines.append("")
    
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- FEES")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE fees (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    \"desc\" VARCHAR,")
    sql_lines.append("    amount INTEGER DEFAULT 0")
    sql_lines.append(");")
    sql_lines.append("")
    sql_lines.append("INSERT INTO fees (id, \"desc\", amount) VALUES")
    sql_lines.append("(1, 'Annual adult subscription', 1500),")
    sql_lines.append("(2, 'Annual youth subscription', 800),")
    sql_lines.append("(3, 'Annual family subscription', 2500),")
    sql_lines.append("(4, 'Temporary card (3 months)', 500);")
    sql_lines.append("")
    sql_lines.append("SELECT setval('fees_id_seq', 10);")
    sql_lines.append("")
    
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- REMOTE ITEMS")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE TABLE remote_items (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    media_type VARCHAR,")
    sql_lines.append("    identification VARCHAR,")
    sql_lines.append("    price VARCHAR,")
    sql_lines.append("    barcode VARCHAR,")
    sql_lines.append("    dewey VARCHAR,")
    sql_lines.append("    publication_date VARCHAR,")
    sql_lines.append("    lang SMALLINT,")
    sql_lines.append("    lang_orig SMALLINT,")
    sql_lines.append("    title1 VARCHAR,")
    sql_lines.append("    title2 VARCHAR,")
    sql_lines.append("    title3 VARCHAR,")
    sql_lines.append("    title4 VARCHAR,")
    sql_lines.append("    author1_ids INTEGER[],")
    sql_lines.append("    author1_functions VARCHAR,")
    sql_lines.append("    author2_ids INTEGER[],")
    sql_lines.append("    author2_functions VARCHAR,")
    sql_lines.append("    author3_ids INTEGER[],")
    sql_lines.append("    author3_functions VARCHAR,")
    sql_lines.append("    serie_id INTEGER,")
    sql_lines.append("    serie_vol_number SMALLINT,")
    sql_lines.append("    collection_id INTEGER,")
    sql_lines.append("    collection_number_sub SMALLINT,")
    sql_lines.append("    collection_vol_number SMALLINT,")
    sql_lines.append("    source_id INTEGER,")
    sql_lines.append("    source_date VARCHAR,")
    sql_lines.append("    source_norme VARCHAR,")
    sql_lines.append("    genre SMALLINT,")
    sql_lines.append("    subject VARCHAR,")
    sql_lines.append("    public_type SMALLINT,")
    sql_lines.append("    edition_id INTEGER,")
    sql_lines.append("    edition_date VARCHAR,")
    sql_lines.append("    nb_pages VARCHAR,")
    sql_lines.append("    format VARCHAR,")
    sql_lines.append("    content VARCHAR,")
    sql_lines.append("    addon VARCHAR,")
    sql_lines.append("    abstract VARCHAR,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    keywords VARCHAR,")
    sql_lines.append("    nb_specimens SMALLINT,")
    sql_lines.append("    state VARCHAR,")
    sql_lines.append("    is_archive SMALLINT DEFAULT 0,")
    sql_lines.append("    archived_timestamp INTEGER,")
    sql_lines.append("    is_valid SMALLINT DEFAULT 0,")
    sql_lines.append("    modif_date INTEGER,")
    sql_lines.append("    crea_date INTEGER")
    sql_lines.append(");")
    sql_lines.append("")
    
    sql_lines.append("CREATE TABLE remote_specimens (")
    sql_lines.append("    id SERIAL PRIMARY KEY,")
    sql_lines.append("    id_item INTEGER,")
    sql_lines.append("    source_id INTEGER,")
    sql_lines.append("    identification VARCHAR,")
    sql_lines.append("    cote VARCHAR,")
    sql_lines.append("    media_type VARCHAR,")
    sql_lines.append("    place SMALLINT,")
    sql_lines.append("    status SMALLINT,")
    sql_lines.append("    codestat SMALLINT,")
    sql_lines.append("    notes VARCHAR,")
    sql_lines.append("    price VARCHAR,")
    sql_lines.append("    creation_date INTEGER,")
    sql_lines.append("    modif_date INTEGER")
    sql_lines.append(");")
    sql_lines.append("")
    
    # Indexes
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- INDEXES")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("CREATE INDEX users_id_key ON users (id);")
    sql_lines.append("CREATE INDEX users_login_key ON users (login);")
    sql_lines.append("CREATE INDEX authors_id_key ON authors (id);")
    sql_lines.append("CREATE INDEX authors_lastname_key ON authors (lastname);")
    sql_lines.append("CREATE INDEX editions_id_key ON editions (id);")
    sql_lines.append("CREATE INDEX items_id_key ON items (id);")
    sql_lines.append("CREATE INDEX items_identification_key ON items (identification);")
    sql_lines.append("CREATE INDEX items_title1_key ON items (title1);")
    sql_lines.append("CREATE INDEX specimens_id_key ON specimens (id);")
    sql_lines.append("CREATE INDEX specimens_id_item_key ON specimens (id_item);")
    sql_lines.append("CREATE INDEX specimens_identification_key ON specimens (identification);")
    sql_lines.append("CREATE INDEX borrows_id_key ON borrows (id);")
    sql_lines.append("CREATE INDEX borrows_user_id_key ON borrows (user_id);")
    sql_lines.append("CREATE INDEX borrows_specimen_id_key ON borrows (specimen_id);")
    sql_lines.append("")
    
    # Summary
    sql_lines.append("-- =============================================================================")
    sql_lines.append("-- END")
    sql_lines.append("-- =============================================================================")
    sql_lines.append("")
    sql_lines.append("DO $$")
    sql_lines.append("BEGIN")
    sql_lines.append("    RAISE NOTICE '';")
    sql_lines.append("    RAISE NOTICE '=== Elidune legacy large dataset created successfully ===';")
    sql_lines.append("    RAISE NOTICE '';")
    sql_lines.append("    RAISE NOTICE 'Statistics:';")
    sql_lines.append(f"    RAISE NOTICE '  - Users: {num_users}';")
    sql_lines.append(f"    RAISE NOTICE '  - Authors: {num_authors}';")
    total_loans = len(current_loans) + len(returned_loans)
    sql_lines.append(f"    RAISE NOTICE '  - Items: {num_items}';")
    sql_lines.append(f"    RAISE NOTICE '  - Specimens: {num_specimens}';")
    sql_lines.append(f"    RAISE NOTICE '  - Current loans: {len(current_loans)}';")
    sql_lines.append(f"    RAISE NOTICE '  - Returned loans: {len(returned_loans)}';")
    sql_lines.append(f"    RAISE NOTICE '  - Total loans: {total_loans}';")
    sql_lines.append("    RAISE NOTICE '';")
    sql_lines.append("END $$;")
    sql_lines.append("")
    
    # Write to file
    output_file = "/home/cjean/Documents/Developments/elidune/elidune-server-rust/scripts/large_legacy_data.sql"
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write('\n'.join(sql_lines))
    
    print(f"\nSQL file generated: {output_file}")
    print(f"  - Users: {num_users}")
    print(f"  - Authors: {num_authors}")
    print(f"  - Items: {num_items}")
    print(f"  - Specimens: {num_specimens}")
    print(f"  - Current loans: {len(current_loans)}")
    print(f"  - Returned loans: {len(returned_loans)}")
    print(f"  - Total loans: {total_loans}")


if __name__ == "__main__":
    main()
