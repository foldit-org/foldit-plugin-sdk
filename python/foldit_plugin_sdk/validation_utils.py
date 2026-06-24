"""Input validation utilities for foldit-runner plugins.

This module provides common validation functions for model inputs
like amino acid sequences, temperatures, and iteration counts.

Usage:
    from validation_utils import validate_sequence, validate_num_steps

    validate_sequence(sequence)  # Raises ValueError if invalid
    validate_num_steps(num_steps, min_val=50, max_val=1000)
"""

# Standard amino acid codes (20 canonical amino acids)
VALID_AMINO_ACIDS = set("ACDEFGHIKLMNPQRSTVWY")


def validate_sequence(sequence, max_length=None):
    """Validate an amino acid sequence.

    Args:
        sequence: Amino acid sequence string
        max_length: Optional maximum length (raises ValueError if exceeded)

    Raises:
        ValueError: If sequence is empty, contains invalid characters,
                   or exceeds max_length

    Example:
        validate_sequence("MKTAYIAKQRQISFVK")  # OK
        validate_sequence("MKTX")  # Raises ValueError (X is invalid)
        validate_sequence("", max_length=100)  # Raises ValueError (empty)
    """
    if not sequence:
        raise ValueError("Sequence cannot be empty")

    sequence_upper = sequence.upper()
    invalid_chars = set(sequence_upper) - VALID_AMINO_ACIDS

    if invalid_chars:
        raise ValueError(
            f"Invalid amino acids in sequence: {sorted(invalid_chars)}. "
            f"Valid amino acids are: {''.join(sorted(VALID_AMINO_ACIDS))}"
        )

    if max_length is not None and len(sequence) > max_length:
        raise ValueError(
            f"Sequence length {len(sequence)} exceeds maximum {max_length}"
        )


def validate_num_steps(num_steps, min_val, max_val, param_name="num_steps"):
    """Validate a numeric parameter is within a valid range.

    Args:
        num_steps: Value to validate
        min_val: Minimum allowed value (inclusive)
        max_val: Maximum allowed value (inclusive)
        param_name: Parameter name for error messages

    Raises:
        ValueError: If value is outside the valid range

    Example:
        validate_num_steps(500, min_val=50, max_val=1000)  # OK
        validate_num_steps(10, min_val=50, max_val=1000)  # Raises ValueError
    """
    if num_steps < min_val or num_steps > max_val:
        raise ValueError(
            f"{param_name} must be between {min_val} and {max_val}, got {num_steps}"
        )


def validate_temperature(temperature, min_val=0.0, max_val=2.0):
    """Validate a temperature parameter for sampling.

    Args:
        temperature: Temperature value
        min_val: Minimum allowed value (default 0.0)
        max_val: Maximum allowed value (default 2.0)

    Raises:
        ValueError: If temperature is outside the valid range

    Example:
        validate_temperature(0.1)  # OK
        validate_temperature(-0.5)  # Raises ValueError
    """
    if temperature < min_val or temperature > max_val:
        raise ValueError(
            f"Temperature must be between {min_val} and {max_val}, got {temperature}"
        )


def validate_positive_int(value, param_name="value"):
    """Validate that a value is a positive integer.

    Args:
        value: Value to validate
        param_name: Parameter name for error messages

    Raises:
        ValueError: If value is not a positive integer

    Example:
        validate_positive_int(10, "num_designs")  # OK
        validate_positive_int(0, "num_designs")  # Raises ValueError
    """
    if not isinstance(value, int) or value <= 0:
        raise ValueError(f"{param_name} must be a positive integer, got {value}")
