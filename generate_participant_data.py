#!/usr/bin/env python3
"""
Participant Data Generator Script
Generates random participant data for testing the ActionAid SwiftUI app
"""

import json
import random
import requests
import uuid
from datetime import datetime, timedelta
from typing import List, Dict, Any

# Configuration
BASE_URL = "http://localhost:8080"  # Adjust to your server URL
TOTAL_PARTICIPANTS = 50  # Number of participants to create

# Sample data pools - ADD MORE DATA HERE AS NEEDED
FIRST_NAMES = [
    # African names
    "Amara", "Khadija", "Fatou", "Aisha", "Zeinab", "Mariam", "Safiya", "Halima", "Nuru", "Amina",
    "Kwame", "Kofi", "Yaw", "Akwasi", "Kweku", "Kwabena", "Kwadwo", "Kojo", "Fiifi", "Kweku",
    "Asha", "Dalila", "Fadhila", "Hadiya", "Jamila", "Kamaria", "Layla", "Nadia", "Rashida", "Zahara",
    "Abdi", "Ahmed", "Ali", "Hassan", "Ibrahim", "Khalid", "Mohammed", "Omar", "Rashid", "Yusuf",
    
    # Add your own names here
    "Grace", "Hope", "Faith", "Joy", "Peace", "Patience", "Mercy", "Love", "Charity", "Comfort",
    "Emmanuel", "Samuel", "David", "John", "Paul", "Peter", "Michael", "Joseph", "Daniel", "James"
]

LAST_names = [
    # African surnames
    "Okonkwo", "Adebayo", "Okafor", "Emeka", "Chioma", "Ngozi", "Kemi", "Tunde", "Folake", "Segun",
    "Mensah", "Asante", "Boateng", "Owusu", "Adjei", "Appiah", "Darko", "Bediako", "Sarpong", "Agyei",
    "Hassan", "Mohamed", "Ahmed", "Ali", "Ibrahim", "Osman", "Abdulla", "Farah", "Aden", "Sheikh",
    "Temba", "Ndovu", "Simba", "Jabari", "Jengo", "Kamau", "Mwangi", "Njoroge", "Wanjiku", "Wairimu",
    
    # Add your own surnames here
    "Johnson", "Williams", "Brown", "Davis", "Miller", "Wilson", "Moore", "Taylor", "Anderson", "Thomas"
]

LOCATIONS = [
    # African locations
    "Nairobi", "Lagos", "Accra", "Kampala", "Dar es Salaam", "Kigali", "Addis Ababa", "Dakar", "Bamako", "Ouagadougou",
    "Kibera", "Mathare", "Kawangware", "Mukuru", "Dandora", "Korogocho", "Viwandani", "Gitathuru", "Kariobangi", "Huruma",
    "Mushin", "Agege", "Ikeja", "Surulere", "Yaba", "Ikoyi", "Victoria Island", "Lekki", "Ajah", "Alaba",
    "Kasoa", "Tema", "Kumasi", "Tamale", "Bolgatanga", "Wa", "Sunyani", "Koforidua", "Cape Coast", "Takoradi",
    
    # Add your own locations here
    "Rural Village A", "Rural Village B", "Community Center District", "Market Area", "School District",
    "Health Center Area", "Agricultural Zone", "Trading Post", "Riverside Community", "Mountain Village"
]

DISABILITY_TYPES = ["visual", "hearing", "physical", "intellectual", "psychosocial", "multiple", "other"]
GENDERS = ["male", "female", "other", "prefer_not_to_say"]
AGE_GROUPS = ["child", "youth", "adult", "elderly"]
SYNC_PRIORITIES = ["low", "normal", "high"]

def generate_participant() -> Dict[str, Any]:
    """Generate a single random participant"""
    first_name = random.choice(FIRST_NAMES)
    last_name = random.choice(LAST_names)
    name = f"{first_name} {last_name}"
    
    has_disability = random.choice([True, False, False, False])  # 25% chance of disability
    disability_type = random.choice(DISABILITY_TYPES) if has_disability else None
    
    # Sometimes leave fields empty to test missing data scenarios
    gender = random.choice(GENDERS + [None, None])  # 25% chance of None
    age_group = random.choice(AGE_GROUPS + [None])  # 20% chance of None
    location = random.choice(LOCATIONS + [None, None])  # 25% chance of None
    
    return {
        "name": name,
        "gender": gender,
        "disability": has_disability,
        "disability_type": disability_type,
        "age_group": age_group,
        "location": location,
        "sync_priority": random.choice(SYNC_PRIORITIES)
    }

def create_participant_via_api(participant_data: Dict[str, Any], auth_context: Dict[str, Any]) -> bool:
    """Create a participant via the API"""
    payload = {
        "participant": participant_data,
        "auth": auth_context
    }
    
    try:
        response = requests.post(
            f"{BASE_URL}/api/participants",
            json=payload,
            headers={"Content-Type": "application/json"},
            timeout=10
        )
        
        if response.status_code == 200:
            result = response.json()
            print(f"✅ Created participant: {participant_data['name']}")
            return True
        else:
            print(f"❌ Failed to create {participant_data['name']}: {response.status_code} - {response.text}")
            return False
            
    except Exception as e:
        print(f"❌ Error creating {participant_data['name']}: {str(e)}")
        return False

def generate_auth_context() -> Dict[str, Any]:
    """Generate a mock auth context - ADJUST THESE VALUES"""
    return {
        "user_id": "21b04211-3e32-4919-8c80-d7913c04ee3c",  # Use your actual user ID
        "role": "admin",
        "device_id": str(uuid.uuid4()),
        "offline_mode": False
    }

def print_sample_json():
    """Print sample participant JSON for manual testing"""
    print("\n" + "="*60)
    print("SAMPLE PARTICIPANT JSON (for manual testing):")
    print("="*60)
    
    for i in range(5):
        participant = generate_participant()
        auth = generate_auth_context()
        payload = {
            "participant": participant,
            "auth": auth
        }
        print(f"\n// Participant {i+1}")
        print(json.dumps(payload, indent=2))

def main():
    print("🚀 ActionAid Participant Data Generator")
    print(f"📊 Generating {TOTAL_PARTICIPANTS} random participants...")
    print(f"🌐 Target URL: {BASE_URL}")
    
    # Generate auth context
    auth_context = generate_auth_context()
    print(f"👤 Using user ID: {auth_context['user_id']}")
    
    # Track statistics
    created_count = 0
    failed_count = 0
    
    # Generate participants
    for i in range(TOTAL_PARTICIPANTS):
        participant = generate_participant()
        
        print(f"\n📝 [{i+1}/{TOTAL_PARTICIPANTS}] Creating: {participant['name']}")
        print(f"    Gender: {participant.get('gender', 'Not specified')}")
        print(f"    Age Group: {participant.get('age_group', 'Not specified')}")
        print(f"    Location: {participant.get('location', 'Not specified')}")
        print(f"    Disability: {'Yes' if participant.get('disability') else 'No'}")
        
        if create_participant_via_api(participant, auth_context):
            created_count += 1
        else:
            failed_count += 1
        
        # Add small delay to avoid overwhelming the server
        import time
        time.sleep(0.1)
    
    # Print summary
    print("\n" + "="*60)
    print("📊 GENERATION SUMMARY")
    print("="*60)
    print(f"✅ Successfully created: {created_count}")
    print(f"❌ Failed to create: {failed_count}")
    print(f"📈 Success rate: {(created_count/TOTAL_PARTICIPANTS)*100:.1f}%")
    
    # Print sample JSON for manual use
    print_sample_json()

if __name__ == "__main__":
    main() 