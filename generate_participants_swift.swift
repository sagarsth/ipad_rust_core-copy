#!/usr/bin/env swift

import Foundation

// MARK: - Data Models (simplified)
struct NewParticipant: Codable {
    let name: String
    let gender: String?
    let disability: Bool?
    let disabilityType: String?
    let ageGroup: String?
    let location: String?
    let createdByUserId: String?
    let syncPriority: String
    
    enum CodingKeys: String, CodingKey {
        case name, gender, disability, location
        case disabilityType = "disability_type"
        case ageGroup = "age_group"
        case createdByUserId = "created_by_user_id"
        case syncPriority = "sync_priority"
    }
}

struct AuthContext: Codable {
    let userId: String
    let role: String
    let deviceId: String
    let offlineMode: Bool
    
    enum CodingKeys: String, CodingKey {
        case userId = "user_id"
        case role
        case deviceId = "device_id"
        case offlineMode = "offline_mode"
    }
}

struct ParticipantCreateRequest: Codable {
    let participant: NewParticipant
    let auth: AuthContext
}

// MARK: - Sample Data
let firstNames = [
    "Amara", "Khadija", "Fatou", "Aisha", "Zeinab", "Mariam", "Safiya", "Halima", "Nuru", "Amina",
    "Kwame", "Kofi", "Yaw", "Akwasi", "Kweku", "Kwabena", "Kwadwo", "Kojo", "Fiifi",
    "Asha", "Dalila", "Fadhila", "Hadiya", "Jamila", "Kamaria", "Layla", "Nadia", "Rashida", "Zahara",
    "Grace", "Hope", "Faith", "Joy", "Peace", "Patience", "Mercy", "Love", "Charity", "Comfort",
    "Emmanuel", "Samuel", "David", "John", "Paul", "Peter", "Michael", "Joseph", "Daniel", "James"
]

let lastNames = [
    "Okonkwo", "Adebayo", "Okafor", "Emeka", "Chioma", "Ngozi", "Kemi", "Tunde", "Folake", "Segun",
    "Mensah", "Asante", "Boateng", "Owusu", "Adjei", "Appiah", "Darko", "Bediako", "Sarpong", "Agyei",
    "Hassan", "Mohamed", "Ahmed", "Ali", "Ibrahim", "Osman", "Abdulla", "Farah", "Aden", "Sheikh",
    "Johnson", "Williams", "Brown", "Davis", "Miller", "Wilson", "Moore", "Taylor", "Anderson", "Thomas"
]

let locations = [
    "Nairobi", "Lagos", "Accra", "Kampala", "Dar es Salaam", "Kigali", "Addis Ababa", "Dakar",
    "Kibera", "Mathare", "Kawangware", "Mukuru", "Dandora", "Korogocho", "Viwandani",
    "Mushin", "Agege", "Ikeja", "Surulere", "Yaba", "Ikoyi", "Victoria Island",
    "Rural Village A", "Rural Village B", "Community Center District", "Market Area", "School District"
]

let genders = ["male", "female", "other", "prefer_not_to_say"]
let ageGroups = ["child", "youth", "adult", "elderly"]
let disabilityTypes = ["visual", "hearing", "physical", "intellectual", "psychosocial", "multiple", "other"]

// MARK: - Helper Functions
func randomChoice<T>(_ array: [T]) -> T {
    return array.randomElement()!
}

func randomOptional<T>(_ array: [T], nilChance: Double = 0.25) -> T? {
    return Double.random(in: 0...1) < nilChance ? nil : randomChoice(array)
}

func generateParticipant() -> NewParticipant {
    let firstName = randomChoice(firstNames)
    let lastName = randomChoice(lastNames)
    let name = "\(firstName) \(lastName)"
    
    let hasDisability = Double.random(in: 0...1) < 0.25 // 25% chance
    let disabilityType = hasDisability ? randomChoice(disabilityTypes) : nil
    
    return NewParticipant(
        name: name,
        gender: randomOptional(genders),
        disability: hasDisability,
        disabilityType: disabilityType,
        ageGroup: randomOptional(ageGroups),
        location: randomOptional(locations),
        createdByUserId: "21b04211-3e32-4919-8c80-d7913c04ee3c", // Your user ID
        syncPriority: randomChoice(["low", "normal", "high"])
    )
}

func generateAuthContext() -> AuthContext {
    return AuthContext(
        userId: "21b04211-3e32-4919-8c80-d7913c04ee3c", // Your user ID
        role: "admin",
        deviceId: UUID().uuidString,
        offlineMode: false
    )
}

// MARK: - Main Function
func main() {
    print("🚀 Swift Participant Data Generator")
    print("📱 This generates JSON payloads for manual testing")
    print("=" + String(repeating: "=", count: 50))
    
    let totalParticipants = 10 // Adjust as needed
    let auth = generateAuthContext()
    
    for i in 1...totalParticipants {
        let participant = generateParticipant()
        let request = ParticipantCreateRequest(participant: participant, auth: auth)
        
        print("\n// Participant \(i)")
        print("// Name: \(participant.name)")
        print("// Gender: \(participant.gender ?? "Not specified")")
        print("// Age Group: \(participant.ageGroup ?? "Not specified")")
        print("// Location: \(participant.location ?? "Not specified")")
        print("// Disability: \(participant.disability == true ? "Yes" : "No")")
        
        do {
            let encoder = JSONEncoder()
            encoder.keyEncodingStrategy = .convertToSnakeCase
            let jsonData = try encoder.encode(request)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            print(jsonString)
        } catch {
            print("❌ Error encoding participant \(i): \(error)")
        }
    }
    
    print("\n" + String(repeating: "=", count: 60))
    print("📋 COPY THE JSON ABOVE AND USE IT TO:")
    print("1. Test your FFI handlers manually")
    print("2. Add data via your app's UI")
    print("3. Debug the participant creation flow")
    print(String(repeating: "=", count: 60))
}

// Run the main function
main() 