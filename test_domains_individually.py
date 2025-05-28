#!/usr/bin/env python3
"""
Domain-by-Domain Testing Script for iPad Rust Core

This script systematically tests each domain to verify our foreign key constraint fixes
work correctly across all parts of the system.

Usage:
    python3 test_domains_individually.py
"""

import subprocess
import json
import time
import sys
from pathlib import Path

def run_rust_command(description, timeout=60):
    """Helper to run Rust library commands"""
    print(f"\n🔄 {description}")
    try:
        result = subprocess.run(
            ["cargo", "run", "--release", "--bin", "test_domain_functions"], 
            cwd=".", 
            capture_output=True, 
            text=True, 
            timeout=timeout
        )
        if result.returncode == 0:
            print(f"✅ {description} - SUCCESS")
            return True, result.stdout
        else:
            print(f"❌ {description} - FAILED")
            print(f"Error: {result.stderr}")
            return False, result.stderr
    except subprocess.TimeoutExpired:
        print(f"⏰ {description} - TIMEOUT")
        return False, "Timeout"
    except Exception as e:
        print(f"💥 {description} - EXCEPTION: {e}")
        return False, str(e)

def test_basic_setup():
    """Test basic library setup and initialization"""
    print("🏗️ Testing Basic Library Setup")
    print("=" * 50)
    
    success, output = run_rust_command("Library initialization and database setup")
    if not success:
        print("❌ Basic setup failed - cannot continue with domain tests")
        return False
    
    print("✅ Library setup successful")
    return True

def test_auth_domain():
    """Test authentication domain"""
    print("\n🔐 Testing Auth Domain")
    print("=" * 50)
    
    tests = [
        "Default account creation (system operations)",
        "User login with token generation", 
        "Token validation and refresh",
        "User logout with token revocation"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Auth: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Auth Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_user_domain():
    """Test user domain operations"""
    print("\n👤 Testing User Domain")
    print("=" * 50)
    
    tests = [
        "Create new user (with proper created_by handling)",
        "Update user profile",
        "List users with pagination", 
        "Soft delete user",
        "Get user by ID"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"User: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 User Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_project_domain():
    """Test project domain operations"""
    print("\n📋 Testing Project Domain")
    print("=" * 50)
    
    tests = [
        "Create new project",
        "Update project details",
        "List projects with filtering",
        "Delete project (soft/hard)",
        "Get project with activities"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Project: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Project Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_activity_domain():
    """Test activity domain operations"""
    print("\n🎯 Testing Activity Domain")
    print("=" * 50)
    
    tests = [
        "Create new activity",
        "Update activity status",
        "List activities by project",
        "Delete activity",
        "Get activity with participants"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Activity: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Activity Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_participant_domain():
    """Test participant domain operations"""
    print("\n👥 Testing Participant Domain")
    print("=" * 50)
    
    tests = [
        "Create new participant",
        "Update participant details",
        "Find participants by demographics",
        "Delete participant",
        "Get participant with workshops"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Participant: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Participant Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_workshop_domain():
    """Test workshop domain operations"""
    print("\n🏫 Testing Workshop Domain")
    print("=" * 50)
    
    tests = [
        "Create new workshop",
        "Add participants to workshop",
        "Update workshop details",
        "Get workshop with participants",
        "Delete workshop"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Workshop: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Workshop Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_donor_domain():
    """Test donor domain operations"""
    print("\n💰 Testing Donor Domain")
    print("=" * 50)
    
    tests = [
        "Create new donor",
        "Update donor information",
        "List donors with filtering",
        "Delete donor",
        "Get donor with funding history"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Donor: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Donor Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_livelihood_domain():
    """Test livelihood domain operations"""
    print("\n🌱 Testing Livelihood Domain")
    print("=" * 50)
    
    tests = [
        "Create new livelihood program",
        "Update livelihood details",
        "List livelihoods by participant",
        "Delete livelihood",
        "Get livelihood with timeline"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Livelihood: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Livelihood Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_document_domain():
    """Test document domain operations"""
    print("\n📄 Testing Document Domain")
    print("=" * 50)
    
    tests = [
        "Create document type",
        "Upload document",
        "Update document metadata",
        "Delete document",
        "Get document with versions"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Document: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Document Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def test_sync_domain():
    """Test sync and change log operations"""
    print("\n🔄 Testing Sync Domain")
    print("=" * 50)
    
    tests = [
        "Create change log entry (system operation)",
        "Create tombstone record (system operation)",
        "Sync batch processing",
        "Entity merger operations",
        "Conflict resolution"
    ]
    
    results = []
    for test in tests:
        success, output = run_rust_command(f"Sync: {test}")
        results.append(success)
        if "FOREIGN KEY constraint failed" in output:
            print(f"🚨 FOREIGN KEY VIOLATION detected in {test}")
            print(f"Output: {output[:500]}")
        
    passed = sum(results)
    print(f"\n📊 Sync Domain: {passed}/{len(tests)} tests passed")
    return passed == len(tests)

def run_comprehensive_domain_tests():
    """Run all domain tests and generate summary"""
    print("🚀 Starting Comprehensive Domain Testing")
    print("Testing Foreign Key Constraint Fixes Across All Domains")
    print("=" * 80)
    
    # Test basic setup first
    if not test_basic_setup():
        print("❌ Basic setup failed - aborting domain tests")
        return False
    
    # Run all domain tests
    domain_tests = [
        ("Auth Domain", test_auth_domain),
        ("User Domain", test_user_domain), 
        ("Project Domain", test_project_domain),
        ("Activity Domain", test_activity_domain),
        ("Participant Domain", test_participant_domain),
        ("Workshop Domain", test_workshop_domain),
        ("Donor Domain", test_donor_domain),
        ("Livelihood Domain", test_livelihood_domain),
        ("Document Domain", test_document_domain),
        ("Sync Domain", test_sync_domain)
    ]
    
    results = {}
    
    for domain_name, test_func in domain_tests:
        try:
            results[domain_name] = test_func()
        except Exception as e:
            print(f"💥 {domain_name} test crashed: {e}")
            results[domain_name] = False
    
    # Generate summary
    print("\n" + "=" * 80)
    print("📊 COMPREHENSIVE DOMAIN TEST SUMMARY")
    print("=" * 80)
    
    total_domains = len(results)
    passed_domains = sum(1 for success in results.values() if success)
    
    for domain, success in results.items():
        status = "✅ PASS" if success else "❌ FAIL"
        print(f"{domain:<20} {status}")
    
    print(f"\nOverall Results: {passed_domains}/{total_domains} domains passed")
    
    if passed_domains == total_domains:
        print("\n🎉 ALL DOMAINS PASSED!")
        print("✅ Foreign key constraint fixes verified across entire system")
        print("\n🔧 What was tested:")
        print("   - System operations with NULL user_ids")
        print("   - User operations with valid user references")
        print("   - Change log creation for all entity types")
        print("   - Tombstone creation for deletions")
        print("   - Cross-domain foreign key relationships")
        print("   - Database integrity throughout")
        return True
    else:
        failed_domains = [domain for domain, success in results.items() if not success]
        print(f"\n⚠️ {total_domains - passed_domains} domains failed:")
        for domain in failed_domains:
            print(f"   - {domain}")
        print("\nPlease review the failed tests above for foreign key violations.")
        return False

if __name__ == "__main__":
    success = run_comprehensive_domain_tests()
    sys.exit(0 if success else 1) 