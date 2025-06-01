use guacamole::combinators::any;
use guacamole::Guacamole;

#[test]
fn test_any_f64_uniform_distribution() {
    let mut guac = Guacamole::default();
    let sample_size = 100_000;
    let num_bins = 100;
    let mut bins = vec![0; num_bins];
    
    // Generate samples and put them into bins
    for _ in 0..sample_size {
        let value: f64 = any(&mut guac);
        assert!(value >= 0.0 && value < 1.0, "Value should be in [0, 1): {}", value);
        
        let bin_index = (value * num_bins as f64) as usize;
        let bin_index = bin_index.min(num_bins - 1); // Handle edge case where value is exactly 1.0
        bins[bin_index] += 1;
    }
    
    // Expected count per bin for uniform distribution
    let expected_per_bin = sample_size / num_bins;
    let tolerance = (expected_per_bin as f64 * 0.1) as usize; // 10% tolerance
    
    // Check that each bin is within tolerance of expected count
    for (i, &count) in bins.iter().enumerate() {
        let diff = if count > expected_per_bin {
            count - expected_per_bin
        } else {
            expected_per_bin - count
        };
        
        assert!(
            diff <= tolerance,
            "Bin {} has count {} which differs by {} from expected {} (tolerance: {})",
            i, count, diff, expected_per_bin, tolerance
        );
    }
}

#[test]
fn test_any_f64_chi_square_uniformity() {
    let mut guac = Guacamole::default();
    let sample_size = 50_000;
    let num_bins = 50;
    let mut bins = vec![0; num_bins];
    
    // Generate samples and put them into bins
    for _ in 0..sample_size {
        let value: f64 = any(&mut guac);
        assert!(value >= 0.0 && value < 1.0, "Value should be in [0, 1): {}", value);
        
        let bin_index = (value * num_bins as f64) as usize;
        let bin_index = bin_index.min(num_bins - 1);
        bins[bin_index] += 1;
    }
    
    // Chi-square test for uniformity
    let expected_per_bin = sample_size as f64 / num_bins as f64;
    let mut chi_square = 0.0;
    
    for &observed in &bins {
        let diff = observed as f64 - expected_per_bin;
        chi_square += (diff * diff) / expected_per_bin;
    }
    
    // For 49 degrees of freedom (50 bins - 1) at 95% confidence level,
    // critical value is approximately 66.34
    let critical_value = 66.34;
    
    assert!(
        chi_square < critical_value,
        "Chi-square test failed: {} >= {} (critical value at 95% confidence)",
        chi_square, critical_value
    );
}

#[test] 
fn test_any_f64_range_bounds() {
    let mut guac = Guacamole::default();
    
    // Test that all generated values are in [0, 1)
    for _ in 0..10_000 {
        let value: f64 = any(&mut guac);
        assert!(value >= 0.0, "Value should be >= 0.0: {}", value);
        assert!(value < 1.0, "Value should be < 1.0: {}", value);
    }
}

#[test]
fn test_any_f64_deterministic() {
    // Test that the same seed produces the same sequence
    let mut guac1 = Guacamole::new(12345);
    let mut guac2 = Guacamole::new(12345);
    
    for _ in 0..100 {
        let val1: f64 = any(&mut guac1);
        let val2: f64 = any(&mut guac2);
        assert_eq!(val1, val2, "Same seed should produce same values");
    }
}

#[test]
fn test_any_f64_different_seeds() {
    // Test that different seeds produce different sequences
    let mut guac1 = Guacamole::new(12345);
    let mut guac2 = Guacamole::new(54321);
    
    let mut differences = 0;
    for _ in 0..100 {
        let val1: f64 = any(&mut guac1);
        let val2: f64 = any(&mut guac2);
        if val1 != val2 {
            differences += 1;
        }
    }
    
    // We expect most values to be different with different seeds
    assert!(differences > 90, "Different seeds should produce mostly different values, got {} differences", differences);
}

#[test]
fn test_any_f64_kolmogorov_smirnov() {
    let mut guac = Guacamole::default();
    let sample_size = 1000;
    let mut samples = Vec::with_capacity(sample_size);
    
    // Generate samples
    for _ in 0..sample_size {
        let value: f64 = any(&mut guac);
        samples.push(value);
    }
    
    // Sort samples for KS test
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    // Kolmogorov-Smirnov test for uniformity
    let mut max_diff: f64 = 0.0;
    
    for (i, &value) in samples.iter().enumerate() {
        let empirical_cdf = (i + 1) as f64 / sample_size as f64;
        let theoretical_cdf = value; // For uniform distribution on [0,1)
        let diff = (empirical_cdf - theoretical_cdf).abs();
        max_diff = max_diff.max(diff);
    }
    
    // Critical value for KS test at 95% confidence for n=1000 is approximately 0.043
    let critical_value = 0.043;
    
    assert!(
        max_diff < critical_value,
        "Kolmogorov-Smirnov test failed: max difference {} >= critical value {}",
        max_diff, critical_value
    );
}